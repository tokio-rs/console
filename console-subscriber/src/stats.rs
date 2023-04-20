use crate::{attribute, sync::Mutex, ToProto};
use crossbeam_utils::atomic::AtomicCell;
use hdrhistogram::{
    self,
    serialization::{Serializer, V2Serializer},
};
use std::cmp;
use std::sync::{
    atomic::{AtomicBool, AtomicUsize, Ordering::*},
    Arc,
};
use std::time::{Duration, Instant, SystemTime};
use tracing::span::Id;

use console_api as proto;

/// A type which records whether it has unsent updates.
///
/// If something implementing this trait has been changed since the last time
/// data was sent to a client, it will indicate that it is "dirty". If it has
/// not been changed, it does not have to be included in the current update.
pub(crate) trait Unsent {
    /// Returns `true` if this type has unsent updates, and if it does, clears
    /// the flag indicating there are unsent updates.
    ///
    /// This is called when filtering which stats need to be included in the
    /// current update. If this returns `true`, it will be included, so it
    /// becomes no longer dirty.
    fn take_unsent(&self) -> bool;

    /// Returns `true` if this type has unsent updates, without changing the
    /// flag.
    fn is_unsent(&self) -> bool;
}

// An entity (e.g Task, Resource) that at some point in
// time can be dropped. This generally refers to spans that
// have been closed indicating that a task, async op or a
// resource is not in use anymore
pub(crate) trait DroppedAt {
    fn dropped_at(&self) -> Option<Instant>;
}

/// Anchors an `Instant` with a `SystemTime` timestamp to allow converting
/// monotonic `Instant`s into timestamps that can be sent over the wire.
#[derive(Debug, Clone)]
pub(crate) struct TimeAnchor {
    mono: Instant,
    sys: SystemTime,
}

/// Stats associated with a task.
#[derive(Debug)]
pub(crate) struct TaskStats {
    is_dirty: AtomicBool,
    is_dropped: AtomicBool,
    // task stats
    pub(crate) created_at: Instant,
    dropped_at: Mutex<Option<Instant>>,

    // waker stats
    wakes: AtomicUsize,
    waker_clones: AtomicUsize,
    waker_drops: AtomicUsize,
    self_wakes: AtomicUsize,

    /// Poll durations and other stats.
    poll_stats: PollStats<Histogram>,
}

/// Stats associated with an async operation.
///
/// This shares all of the same fields as [`ResourceStats]`, with the addition
/// of [`PollStats`] tracking when the async operation is polled, and the task
/// ID of the last task to poll the async op.
#[derive(Debug)]
pub(crate) struct AsyncOpStats {
    /// The task ID of the last task to poll this async op.
    ///
    /// This is set every time the async op is polled, in case a future is
    /// passed between tasks.
    task_id: AtomicCell<u64>,

    /// Fields shared with `ResourceStats`.
    pub(crate) stats: ResourceStats,

    /// Poll durations and other stats.
    poll_stats: PollStats<()>,
}

/// Stats associated with a resource.
#[derive(Debug)]
pub(crate) struct ResourceStats {
    is_dirty: AtomicBool,
    is_dropped: AtomicBool,
    created_at: Instant,
    dropped_at: Mutex<Option<Instant>>,
    attributes: Mutex<attribute::Attributes>,
    pub(crate) inherit_child_attributes: bool,
    pub(crate) parent_id: Option<Id>,
}

#[derive(Debug, Default)]
struct PollStats<H> {
    /// The number of polls in progress
    current_polls: AtomicUsize,
    /// The total number of polls
    polls: AtomicUsize,
    timestamps: Mutex<PollTimestamps<H>>,
}

#[derive(Debug, Default)]
struct PollTimestamps<H> {
    first_poll: Option<Instant>,
    last_wake: Option<Instant>,
    last_poll_started: Option<Instant>,
    last_poll_ended: Option<Instant>,
    busy_time: Duration,
    scheduled_time: Duration,
    poll_histogram: H,
    scheduled_histogram: H,
}

#[derive(Debug)]
struct Histogram {
    histogram: hdrhistogram::Histogram<u64>,
    max: u64,
    outliers: u64,
    max_outlier: Option<u64>,
}

trait RecordDuration {
    fn record_duration(&mut self, duration: Duration);
}

impl TimeAnchor {
    pub(crate) fn new() -> Self {
        Self {
            mono: Instant::now(),
            sys: SystemTime::now(),
        }
    }

    pub(crate) fn to_system_time(&self, t: Instant) -> SystemTime {
        let dur = t
            .checked_duration_since(self.mono)
            .unwrap_or_else(|| Duration::from_secs(0));
        self.sys + dur
    }

    pub(crate) fn to_timestamp(&self, t: Instant) -> prost_types::Timestamp {
        self.to_system_time(t).into()
    }
}

impl TaskStats {
    pub(crate) fn new(
        poll_duration_max: u64,
        scheduled_duration_max: u64,
        created_at: Instant,
    ) -> Self {
        Self {
            is_dirty: AtomicBool::new(true),
            is_dropped: AtomicBool::new(false),
            created_at,
            dropped_at: Mutex::new(None),
            poll_stats: PollStats {
                timestamps: Mutex::new(PollTimestamps {
                    poll_histogram: Histogram::new(poll_duration_max),
                    scheduled_histogram: Histogram::new(scheduled_duration_max),
                    first_poll: None,
                    last_wake: None,
                    last_poll_started: None,
                    last_poll_ended: None,
                    busy_time: Duration::new(0, 0),
                    scheduled_time: Duration::new(0, 0),
                }),
                current_polls: AtomicUsize::new(0),
                polls: AtomicUsize::new(0),
            },
            wakes: AtomicUsize::new(0),
            waker_clones: AtomicUsize::new(0),
            waker_drops: AtomicUsize::new(0),
            self_wakes: AtomicUsize::new(0),
        }
    }

    pub(crate) fn record_wake_op(&self, op: crate::WakeOp, at: Instant) {
        use crate::WakeOp;
        match op {
            WakeOp::Clone => {
                self.waker_clones.fetch_add(1, Release);
            }
            WakeOp::Drop => {
                self.waker_drops.fetch_add(1, Release);
            }
            WakeOp::WakeByRef { self_wake } => self.wake(at, self_wake),
            WakeOp::Wake { self_wake } => {
                // Note: `Waker::wake` does *not* call the `drop`
                // implementation, so waking by value doesn't
                // trigger a drop event. so, count this as a `drop`
                // to ensure the task's number of wakers can be
                // calculated as `clones` - `drops`.
                //
                // see
                // https://github.com/rust-lang/rust/blob/673d0db5e393e9c64897005b470bfeb6d5aec61b/library/core/src/task/wake.rs#L211-L212
                self.waker_drops.fetch_add(1, Release);

                self.wake(at, self_wake)
            }
        }
        self.make_dirty();
    }

    fn wake(&self, at: Instant, self_wake: bool) {
        self.poll_stats.wake(at);

        self.wakes.fetch_add(1, Release);
        if self_wake {
            self.wakes.fetch_add(1, Release);
        }

        self.make_dirty();
    }

    pub(crate) fn start_poll(&self, at: Instant) {
        self.poll_stats.start_poll(at);
        self.make_dirty();
    }

    pub(crate) fn end_poll(&self, at: Instant) {
        self.poll_stats.end_poll(at);
        self.make_dirty();
    }

    pub(crate) fn drop_task(&self, dropped_at: Instant) {
        if self.is_dropped.swap(true, AcqRel) {
            // The task was already dropped.
            // TODO(eliza): this could maybe panic in debug mode...
            return;
        }

        let _prev = self.dropped_at.lock().replace(dropped_at);
        debug_assert_eq!(_prev, None, "tried to drop a task twice; this is a bug!");
        self.make_dirty();
    }

    pub(crate) fn poll_duration_histogram(&self) -> proto::tasks::task_details::PollTimesHistogram {
        let hist = self.poll_stats.timestamps.lock().poll_histogram.to_proto();
        proto::tasks::task_details::PollTimesHistogram::Histogram(hist)
    }

    pub(crate) fn scheduled_duration_histogram(&self) -> proto::tasks::DurationHistogram {
        self.poll_stats
            .timestamps
            .lock()
            .scheduled_histogram
            .to_proto()
    }

    #[inline]
    fn make_dirty(&self) {
        self.is_dirty.swap(true, AcqRel);
    }
}

impl ToProto for TaskStats {
    type Output = proto::tasks::Stats;

    fn to_proto(&self, base_time: &TimeAnchor) -> Self::Output {
        let poll_stats = Some(self.poll_stats.to_proto(base_time));
        let timestamps = self.poll_stats.timestamps.lock();
        proto::tasks::Stats {
            poll_stats,
            created_at: Some(base_time.to_timestamp(self.created_at)),
            dropped_at: self.dropped_at.lock().map(|at| base_time.to_timestamp(at)),
            wakes: self.wakes.load(Acquire) as u64,
            waker_clones: self.waker_clones.load(Acquire) as u64,
            self_wakes: self.self_wakes.load(Acquire) as u64,
            waker_drops: self.waker_drops.load(Acquire) as u64,
            last_wake: timestamps.last_wake.map(|at| base_time.to_timestamp(at)),
            scheduled_time: Some(
                timestamps
                    .scheduled_time
                    .try_into()
                    .unwrap_or_else(|error| {
                        eprintln!(
                            "failed to convert `scheduled_time` to protobuf duration: {}",
                            error
                        );
                        Default::default()
                    }),
            ),
        }
    }
}

impl Unsent for TaskStats {
    #[inline]
    fn take_unsent(&self) -> bool {
        self.is_dirty.swap(false, AcqRel)
    }

    fn is_unsent(&self) -> bool {
        self.is_dirty.load(Acquire)
    }
}

impl DroppedAt for TaskStats {
    fn dropped_at(&self) -> Option<Instant> {
        // avoid acquiring the lock if we know we haven't tried to drop this
        // thing yet
        if self.is_dropped.load(Acquire) {
            return *self.dropped_at.lock();
        }

        None
    }
}

// === impl AsyncOpStats ===

impl AsyncOpStats {
    pub(crate) fn new(
        created_at: Instant,
        inherit_child_attributes: bool,
        parent_id: Option<Id>,
    ) -> Self {
        Self {
            task_id: AtomicCell::new(0),
            stats: ResourceStats::new(created_at, inherit_child_attributes, parent_id),
            poll_stats: PollStats::default(),
        }
    }

    pub(crate) fn task_id(&self) -> Option<u64> {
        let id = self.task_id.load();
        if id > 0 {
            Some(id)
        } else {
            None
        }
    }

    pub(crate) fn set_task_id(&self, id: &tracing::span::Id) {
        self.task_id.store(id.into_u64());
        self.make_dirty();
    }

    pub(crate) fn drop_async_op(&self, dropped_at: Instant) {
        self.stats.drop_resource(dropped_at)
    }

    pub(crate) fn start_poll(&self, at: Instant) {
        self.poll_stats.start_poll(at);
        self.make_dirty();
    }

    pub(crate) fn end_poll(&self, at: Instant) {
        self.poll_stats.end_poll(at);
        self.make_dirty();
    }

    #[inline]
    fn make_dirty(&self) {
        self.stats.make_dirty()
    }
}

impl Unsent for AsyncOpStats {
    #[inline]
    fn take_unsent(&self) -> bool {
        self.stats.take_unsent()
    }

    #[inline]
    fn is_unsent(&self) -> bool {
        self.stats.is_unsent()
    }
}

impl DroppedAt for AsyncOpStats {
    fn dropped_at(&self) -> Option<Instant> {
        self.stats.dropped_at()
    }
}

impl ToProto for AsyncOpStats {
    type Output = proto::async_ops::Stats;

    fn to_proto(&self, base_time: &TimeAnchor) -> Self::Output {
        let attributes = self.stats.attributes.lock().values().cloned().collect();
        proto::async_ops::Stats {
            poll_stats: Some(self.poll_stats.to_proto(base_time)),
            created_at: Some(base_time.to_timestamp(self.stats.created_at)),
            dropped_at: self
                .stats
                .dropped_at
                .lock()
                .map(|at| base_time.to_timestamp(at)),
            task_id: self.task_id().map(Into::into),
            attributes,
        }
    }
}

// === impl ResourceStats ===

impl ResourceStats {
    pub(crate) fn new(
        created_at: Instant,
        inherit_child_attributes: bool,
        parent_id: Option<Id>,
    ) -> Self {
        Self {
            is_dirty: AtomicBool::new(true),
            is_dropped: AtomicBool::new(false),
            created_at,
            dropped_at: Mutex::new(None),
            attributes: Default::default(),
            inherit_child_attributes,
            parent_id,
        }
    }

    pub(crate) fn update_attribute(&self, id: &Id, update: &attribute::Update) {
        self.attributes.lock().update(id, update);
        self.make_dirty();
    }

    #[inline]
    pub(crate) fn drop_resource(&self, dropped_at: Instant) {
        if self.is_dropped.swap(true, AcqRel) {
            // The task was already dropped.
            // TODO(eliza): this could maybe panic in debug mode...
            return;
        }

        let mut timestamp = self.dropped_at.lock();
        let _prev = timestamp.replace(dropped_at);
        debug_assert_eq!(
            _prev, None,
            "tried to drop a resource/async op twice; this is a bug!"
        );
        self.make_dirty();
    }

    #[inline]
    fn make_dirty(&self) {
        self.is_dirty.swap(true, AcqRel);
    }
}

impl Unsent for ResourceStats {
    #[inline]
    fn take_unsent(&self) -> bool {
        self.is_dirty.swap(false, AcqRel)
    }

    fn is_unsent(&self) -> bool {
        self.is_dirty.load(Acquire)
    }
}

impl DroppedAt for ResourceStats {
    fn dropped_at(&self) -> Option<Instant> {
        // avoid acquiring the lock if we know we haven't tried to drop this
        // thing yet
        if self.is_dropped.load(Acquire) {
            return *self.dropped_at.lock();
        }

        None
    }
}

impl ToProto for ResourceStats {
    type Output = proto::resources::Stats;

    fn to_proto(&self, base_time: &TimeAnchor) -> Self::Output {
        let attributes = self.attributes.lock().values().cloned().collect();
        proto::resources::Stats {
            created_at: Some(base_time.to_timestamp(self.created_at)),
            dropped_at: self.dropped_at.lock().map(|at| base_time.to_timestamp(at)),
            attributes,
        }
    }
}

// === impl PollStats ===

impl<H: RecordDuration> PollStats<H> {
    fn wake(&self, at: Instant) {
        let mut timestamps = self.timestamps.lock();
        timestamps.last_wake = cmp::max(timestamps.last_wake, Some(at));
    }

    fn start_poll(&self, at: Instant) {
        if self.current_polls.fetch_add(1, AcqRel) > 0 {
            return;
        }

        // We are starting the first poll
        let mut timestamps = self.timestamps.lock();
        if timestamps.first_poll.is_none() {
            timestamps.first_poll = Some(at);
        }

        timestamps.last_poll_started = Some(at);

        self.polls.fetch_add(1, Release);

        // If the last poll ended after the last wake then it was likely
        // a self-wake, so we measure from the end of the last poll instead.
        // This also ensures that `busy_time` and `scheduled_time` don't overlap.
        let scheduled = match std::cmp::max(timestamps.last_wake, timestamps.last_poll_ended) {
            Some(scheduled) => scheduled,
            None => return, // Async operations record polls, but not wakes
        };

        let elapsed = match at.checked_duration_since(scheduled) {
            Some(elapsed) => elapsed,
            None => {
                eprintln!(
                    "possible Instant clock skew detected: a poll's start timestamp \
                    was before the wake time/last poll end timestamp\nwake = {:?}\n  start = {:?}",
                    scheduled, at
                );
                return;
            }
        };

        // if we have a scheduled time histogram, add the timestamp
        timestamps.scheduled_histogram.record_duration(elapsed);

        timestamps.scheduled_time += elapsed;
    }

    fn end_poll(&self, at: Instant) {
        // Are we ending the last current poll?
        if self.current_polls.fetch_sub(1, AcqRel) > 1 {
            return;
        }

        let mut timestamps = self.timestamps.lock();
        let started = match timestamps.last_poll_started {
            Some(last_poll) => last_poll,
            None => {
                eprintln!(
                    "a poll ended, but start timestamp was recorded. \
                     this is probably a `console-subscriber` bug"
                );
                return;
            }
        };

        timestamps.last_poll_ended = Some(at);
        let elapsed = match at.checked_duration_since(started) {
            Some(elapsed) => elapsed,
            None => {
                eprintln!(
                    "possible Instant clock skew detected: a poll's end timestamp \
                    was before its start timestamp\nstart = {:?}\n  end = {:?}",
                    started, at
                );
                return;
            }
        };

        // if we have a poll time histogram, add the timestamp
        timestamps.poll_histogram.record_duration(elapsed);

        timestamps.busy_time += elapsed;
    }
}

impl<H> ToProto for PollStats<H> {
    type Output = proto::PollStats;

    fn to_proto(&self, base_time: &TimeAnchor) -> Self::Output {
        let timestamps = self.timestamps.lock();
        proto::PollStats {
            polls: self.polls.load(Acquire) as u64,
            first_poll: timestamps.first_poll.map(|at| base_time.to_timestamp(at)),
            last_wake: timestamps.last_wake.map(|at| base_time.to_timestamp(at)),
            last_poll_started: timestamps
                .last_poll_started
                .map(|at| base_time.to_timestamp(at)),
            last_poll_ended: timestamps
                .last_poll_ended
                .map(|at| base_time.to_timestamp(at)),
            busy_time: Some(timestamps.busy_time.try_into().unwrap_or_else(|error| {
                eprintln!(
                    "failed to convert `busy_time` to protobuf duration: {}",
                    error
                );
                Default::default()
            })),
            scheduled_time: Some(
                timestamps
                    .scheduled_time
                    .try_into()
                    .unwrap_or_else(|error| {
                        eprintln!(
                            "failed to convert `scheduled_time` to protobuf duration: {}",
                            error
                        );
                        Default::default()
                    }),
            ),
        }
    }
}

// === impl Arc ===

impl<T: DroppedAt> DroppedAt for Arc<T> {
    fn dropped_at(&self) -> Option<Instant> {
        T::dropped_at(self)
    }
}

impl<T: Unsent> Unsent for Arc<T> {
    fn take_unsent(&self) -> bool {
        T::take_unsent(self)
    }

    fn is_unsent(&self) -> bool {
        T::is_unsent(self)
    }
}

impl<T: ToProto> ToProto for Arc<T> {
    type Output = T::Output;
    fn to_proto(&self, base_time: &TimeAnchor) -> T::Output {
        T::to_proto(self, base_time)
    }
}

// === impl Histogram ===

impl Histogram {
    fn new(max: u64) -> Self {
        // significant figures should be in the [0-5] range and memory usage
        // grows exponentially with higher a sigfig
        let histogram = hdrhistogram::Histogram::new_with_max(max, 2).unwrap();
        Self {
            histogram,
            max,
            max_outlier: None,
            outliers: 0,
        }
    }

    fn to_proto(&self) -> proto::tasks::DurationHistogram {
        let mut serializer = V2Serializer::new();
        let mut raw_histogram = Vec::new();
        serializer
            .serialize(&self.histogram, &mut raw_histogram)
            .expect("histogram failed to serialize");
        proto::tasks::DurationHistogram {
            raw_histogram,
            max_value: self.max,
            high_outliers: self.outliers,
            highest_outlier: self.max_outlier,
        }
    }
}

impl RecordDuration for Histogram {
    fn record_duration(&mut self, duration: Duration) {
        let mut duration_ns = duration.as_nanos() as u64;

        // clamp the duration to the histogram's max value
        if duration_ns > self.max {
            self.outliers += 1;
            self.max_outlier = cmp::max(self.max_outlier, Some(duration_ns));
            duration_ns = self.max;
        }

        self.histogram
            .record(duration_ns)
            .expect("duration has already been clamped to histogram max value")
    }
}

impl RecordDuration for () {
    fn record_duration(&mut self, _: Duration) {
        // do nothing
    }
}
