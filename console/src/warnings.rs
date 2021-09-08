use crate::tasks::Task;

pub trait Warn<T>: std::fmt::Debug {
    fn check(&self, val: &T) -> Option<String>;
}

#[derive(Clone, Debug)]
pub(crate) struct SelfWakePercent {
    min_percent: u64,
}

impl SelfWakePercent {
    pub(crate) const DEFAULT_PERCENT: u64 = 50;
    pub(crate) fn new(min_percent: u64) -> Self {
        Self { min_percent }
    }
}

impl Default for SelfWakePercent {
    fn default() -> Self {
        Self::new(Self::DEFAULT_PERCENT)
    }
}

impl Warn<Task> for SelfWakePercent {
    fn check(&self, task: &Task) -> Option<String> {
        let self_wakes = task.self_wake_percent();
        if self_wakes > self.min_percent {
            return Some(format!(
                "has woken itself for more than {}% of its total wakeups ({}%)",
                self.min_percent, self_wakes
            ));
        }

        None
    }
}
