use crate::sync::Mutex;
use hdrhistogram::Histogram;
use std::cmp;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering::*};
use std::time::{Duration, SystemTime};

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

    poll_times_histogram: Histogram<u64>,
    poll_stats: PollStats,
}

#[derive(Default)]
struct TaskTimestamps {
    dropped_at: Option<SystemTime>,
    last_wake: Option<SystemTime>,
}

#[derive(Default)]
pub(crate) struct PollStats {
    /// The number of polls in progress
    current_polls: AtomicUsize,
    /// The total number of polls
    polls: AtomicUsize,
    busy_time_ns: AtomicUsize,
    timestamps: Mutex<PollTimestamps>,
}

#[derive(Default)]
struct PollTimestamps {
    first_poll: Option<SystemTime>,
    last_poll_started: Option<SystemTime>,
    last_poll_ended: Option<SystemTime>,
    histogram: Option<Histogram<u64>>,
}

impl TaskStats {
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
                let elapsed_ns = elapsed.as_nanos().try_into().unwrap_or(u64::MAX);

                // if we have a poll time histogram, add the timestamp
                if let Some(ref mut histogram) = timestamps.histogram {
                    histogram
                        .record(elapsed_ns)
                        .expect("failed to record histogram for some kind of reason");
                }

                self.busy_time_ns.fetch_add(elapsed_ns as usize, Release);
            }
        }
    }
}
