use crate::sync::Mutex;
use hdrhistogram::Histogram;
use std::cmp;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering::*};
use std::time::{Duration, SystemTime};

use console_api as proto;

pub(crate) trait ToProto {
    type Output;
    fn to_proto(&self) -> Self::Output;
}

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
}

#[derive(Debug)]
pub(crate) struct TaskStats {
    dirty: AtomicBool,
    // task stats
    created_at: SystemTime,
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

#[derive(Debug, Default)]
struct AsyncOpStats {
    created_at: SystemTime,
    dropped_at: Option<SystemTime>,
    task_id: Option<Id>,
    poll_stats: PollStats,
    attributes: HashMap<FieldKey, Attribute>,
}

#[derive(Debug, Default)]
pub(crate) struct PollStats {
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
            dirty: AtomicBool::new(true),
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

    pub(crate) fn clone_waker(&self) {
        self.waker_clones.fetch_add(1, Release);
        let _ = self.dirty.compare_exchange(false, true, AcqRel, Acquire);
    }

    pub(crate) fn drop_waker(&self) {
        self.waker_drops.fetch_add(1, Release);
        let _ = self.dirty.compare_exchange(false, true, AcqRel, Acquire);
    }

    pub(crate) fn wake(&self, at: SystemTime, self_wake: bool) {
        let mut timestamps = self.timestamps.lock();
        timestamps.last_wake = cmp::max(timestamps.last_wake, Some(at));
        self.wakes.fetch_add(1, Release);
        if self_wake {
            self.wakes.fetch_add(1, Release);
        }

        let _ = self.dirty.compare_exchange(false, true, AcqRel, Acquire);
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
        self.dirty
            .compare_exchange(true, false, AcqRel, Acquire)
            .is_ok()
    }
}

// === impl PollStats ===

impl PollStats {
    pub(crate) fn start_poll(&self, at: SystemTime) {
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

    pub(crate) fn end_poll(&self, at: SystemTime) {
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
