use crate::{attribute, sync::Mutex, ToProto};
use hdrhistogram::{
    serialization::{Serializer, V2Serializer},
    Histogram,
};
use std::cmp;
use std::sync::{
    atomic::{AtomicBool, AtomicUsize, Ordering::*},
    Arc,
};
use std::time::{Duration, SystemTime};
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
    fn is_unsent(&self) -> bool;
}

// An entity (e.g Task, Resource) that at some point in
// time can be dropped. This generally refers to spans that
// have been closed indicating that a task, async op or a
// resource is not in use anymore
pub(crate) trait DroppedAt {
    fn dropped_at(&self) -> Option<SystemTime>;
}

impl<T: DroppedAt> DroppedAt for Arc<T> {
    fn dropped_at(&self) -> Option<SystemTime> {
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
    fn to_proto(&self) -> T::Output {
        T::to_proto(self)
    }
}

// pub(crate) trait Stats: ToProto {
//     fn dropped_at(&self) -> Option<SystemTime>;

//     fn to_proto_if_unsent(&self) -> Option<<Self as ToProto>::Output> {
//         if self.take_unsent() {
//             Some(self.to_proto)
//         } else {
//             None
//         }
//     }
// }

#[derive(Debug)]
pub(crate) struct TaskStats {
    is_dirty: AtomicBool,
    is_dropped: AtomicBool,
    // task stats
    pub(crate) created_at: SystemTime,
    timestamps: Mutex<TaskTimestamps>,

    // waker stats
    wakes: AtomicUsize,
    waker_clones: AtomicUsize,
    waker_drops: AtomicUsize,
    self_wakes: AtomicUsize,

    poll_stats: PollStats,
}

#[derive(Debug, Default)]
struct TaskTimestamps {
    dropped_at: Option<SystemTime>,
    last_wake: Option<SystemTime>,
}

#[derive(Debug)]
pub(crate) struct AsyncOpStats {
    task_id: AtomicUsize,
    pub(crate) stats: ResourceStats,
    poll_stats: PollStats,
}

#[derive(Debug)]
pub(crate) struct ResourceStats {
    is_dirty: AtomicBool,
    is_dropped: AtomicBool,
    created_at: SystemTime,
    dropped_at: Mutex<Option<SystemTime>>,
    attributes: Mutex<attribute::Attributes>,
    pub(crate) inherit_child_attributes: bool,
    pub(crate) parent_id: Option<Id>,
}

#[derive(Debug, Default)]
struct PollStats {
    /// The number of polls in progress
    current_polls: AtomicUsize,
    /// The total number of polls
    polls: AtomicUsize,
    timestamps: Mutex<PollTimestamps>,
}

#[derive(Debug, Default)]
struct PollTimestamps {
    first_poll: Option<SystemTime>,
    last_poll_started: Option<SystemTime>,
    last_poll_ended: Option<SystemTime>,
    busy_time: Duration,
    histogram: Option<Histogram<u64>>,
}

impl TaskStats {
    pub(crate) fn new(created_at: SystemTime) -> Self {
        // significant figures should be in the [0-5] range and memory usage
        // grows exponentially with higher a sigfig
        let poll_times_histogram = Histogram::<u64>::new(2).unwrap();
        Self {
            is_dirty: AtomicBool::new(true),
            is_dropped: AtomicBool::new(false),
            created_at,
            timestamps: Mutex::new(TaskTimestamps::default()),
            poll_stats: PollStats {
                timestamps: Mutex::new(PollTimestamps {
                    histogram: Some(poll_times_histogram),
                    ..Default::default()
                }),
                ..Default::default()
            },
            wakes: AtomicUsize::new(0),
            waker_clones: AtomicUsize::new(0),
            waker_drops: AtomicUsize::new(0),
            self_wakes: AtomicUsize::new(0),
        }
    }

    pub(crate) fn record_wake_op(&self, op: crate::WakeOp, at: SystemTime) {
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

    fn wake(&self, at: SystemTime, self_wake: bool) {
        let mut timestamps = self.timestamps.lock();
        timestamps.last_wake = cmp::max(timestamps.last_wake, Some(at));
        self.wakes.fetch_add(1, Release);

        if self_wake {
            self.wakes.fetch_add(1, Release);
        }
    }

    pub(crate) fn start_poll(&self, at: SystemTime) {
        self.poll_stats.start_poll(at);
        self.make_dirty();
    }

    pub(crate) fn end_poll(&self, at: SystemTime) {
        self.poll_stats.end_poll(at);
        self.make_dirty();
    }

    pub(crate) fn since_last_poll(&self, now: SystemTime) -> Option<Duration> {
        self.poll_stats.since_last_poll(now)
    }

    pub(crate) fn drop_task(&self, dropped_at: SystemTime) {
        if self.is_dropped.swap(true, AcqRel) {
            // The task was already dropped.
            // TODO(eliza): this could maybe panic in debug mode...
            return;
        }

        let mut timestamps = self.timestamps.lock();
        let _prev = timestamps.dropped_at.replace(dropped_at);
        debug_assert_eq!(_prev, None, "tried to drop a task twice; this is a bug!");
        self.make_dirty();
    }

    #[inline]
    fn make_dirty(&self) {
        self.is_dirty.swap(true, AcqRel);
    }

    pub(crate) fn serialize_histogram(&self) -> Option<Vec<u8>> {
        let poll_timestamps = self.poll_stats.timestamps.lock();
        let histogram = poll_timestamps.histogram.as_ref()?;
        let mut serializer = V2Serializer::new();
        let mut buf = Vec::new();
        serializer.serialize(histogram, &mut buf).ok()?;
        Some(buf)
    }
}

impl ToProto for TaskStats {
    type Output = proto::tasks::Stats;

    fn to_proto(&self) -> Self::Output {
        let poll_stats = Some(self.poll_stats.to_proto());
        let timestamps = self.timestamps.lock();
        proto::tasks::Stats {
            poll_stats,
            created_at: Some(self.created_at.into()),
            dropped_at: timestamps.dropped_at.map(Into::into),
            wakes: self.wakes.load(Acquire) as u64,
            waker_clones: self.waker_clones.load(Acquire) as u64,
            self_wakes: self.self_wakes.load(Acquire) as u64,
            waker_drops: self.waker_drops.load(Acquire) as u64,
            last_wake: timestamps.last_wake.map(Into::into),
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
    fn dropped_at(&self) -> Option<SystemTime> {
        // avoid acquiring the lock if we know we haven't tried to drop this
        // thing yet
        if self.is_dropped.load(Acquire) {
            return self.timestamps.lock().dropped_at;
        }

        None
    }
}

// === impl AsyncOpStats ===

impl AsyncOpStats {
    pub(crate) fn new(
        created_at: SystemTime,
        inherit_child_attributes: bool,
        parent_id: Option<Id>,
    ) -> Self {
        Self {
            task_id: AtomicUsize::new(0),
            stats: ResourceStats::new(created_at, inherit_child_attributes, parent_id),
            poll_stats: PollStats::default(),
        }
    }

    pub(crate) fn task_id(&self) -> Option<u64> {
        let id = self.task_id.load(Acquire);
        if id > 0 {
            Some(id as u64)
        } else {
            None
        }
    }

    pub(crate) fn drop_async_op(&self, dropped_at: SystemTime) {
        self.stats.drop_resource(dropped_at)
    }

    pub(crate) fn start_poll(&self, at: SystemTime) {
        self.poll_stats.start_poll(at);
        self.make_dirty();
    }

    pub(crate) fn end_poll(&self, at: SystemTime) {
        self.poll_stats.end_poll(at);
        self.make_dirty();
    }

    pub(crate) fn since_last_poll(&self, now: SystemTime) -> Option<Duration> {
        self.poll_stats.since_last_poll(now)
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
    fn dropped_at(&self) -> Option<SystemTime> {
        self.stats.dropped_at()
    }
}

impl ToProto for AsyncOpStats {
    type Output = proto::async_ops::Stats;

    fn to_proto(&self) -> Self::Output {
        let attributes = self.stats.attributes.lock().values().cloned().collect();
        proto::async_ops::Stats {
            poll_stats: Some(self.poll_stats.to_proto()),
            created_at: Some(self.stats.created_at.into()),
            dropped_at: self.stats.dropped_at.lock().map(Into::into),
            task_id: self.task_id().map(Into::into),
            attributes,
        }
    }
}

// === impl ResourceStats ===

impl ResourceStats {
    pub(crate) fn new(
        created_at: SystemTime,
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
    pub(crate) fn drop_resource(&self, dropped_at: SystemTime) {
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
    fn dropped_at(&self) -> Option<SystemTime> {
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

    fn to_proto(&self) -> Self::Output {
        let attributes = self.attributes.lock().values().cloned().collect();
        proto::resources::Stats {
            created_at: Some(self.created_at.into()),
            dropped_at: self.dropped_at.lock().map(Into::into),
            attributes,
        }
    }
}

// === impl PollStats ===

impl PollStats {
    fn start_poll(&self, at: SystemTime) {
        if self.current_polls.fetch_add(1, AcqRel) == 0 {
            // We are starting the first poll
            let mut timestamps = self.timestamps.lock();
            if timestamps.first_poll.is_none() {
                timestamps.first_poll = Some(at);
            }

            timestamps.last_poll_started = Some(at);

            self.polls.fetch_add(1, Release);
        }
    }

    fn end_poll(&self, at: SystemTime) {
        if self.current_polls.fetch_sub(1, AcqRel) == 1 {
            // We are ending the last current poll
            let mut timestamps = self.timestamps.lock();
            let last_poll_started = timestamps.last_poll_started;
            debug_assert!(last_poll_started.is_some(), "must have started a poll before ending a poll; this is a `console-subscriber` bug!");
            timestamps.last_poll_ended = Some(at);
            let elapsed = last_poll_started.and_then(|started| at.duration_since(started).ok());
            debug_assert!(elapsed.is_some(), "the current poll must have started before it ended; this is a `console-subscriber` bug!");
            if let Some(elapsed) = elapsed {
                // if we have a poll time histogram, add the timestamp
                if let Some(ref mut histogram) = timestamps.histogram {
                    let elapsed_ns = elapsed.as_nanos().try_into().unwrap_or(u64::MAX);
                    histogram
                        .record(elapsed_ns)
                        .expect("failed to record histogram for some kind of reason");
                }

                timestamps.busy_time += elapsed;
            }
        }
    }

    fn since_last_poll(&self, timestamp: SystemTime) -> Option<Duration> {
        self.timestamps
            .lock()
            .last_poll_started
            .map(|lps| timestamp.duration_since(lps).unwrap())
    }
}

impl ToProto for PollStats {
    type Output = proto::PollStats;

    fn to_proto(&self) -> Self::Output {
        let timestamps = self.timestamps.lock();
        proto::PollStats {
            polls: self.polls.load(Acquire) as u64,
            first_poll: timestamps.first_poll.map(Into::into),
            last_poll_started: timestamps.last_poll_started.map(Into::into),
            last_poll_ended: timestamps.last_poll_ended.map(Into::into),
            busy_time: Some(timestamps.busy_time.into()),
        }
    }
}
