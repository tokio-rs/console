use crate::tasks::Task;
use tui::text::{Span, Spans};

pub trait Warning<T>: std::fmt::Debug {
    fn check(&self, val: &T) -> Option<tui::text::Spans>;
}

#[derive(Clone, Debug)]
pub(crate) struct SelfWakePercent {
    min_percent: f64,
}

impl SelfWakePercent {
    pub(crate) const DEFAULT_PERCENT: f64 = 50.0;
    pub(crate) fn new(min_percent: f64) -> Self {
        Self { min_percent }
    }
}

impl Default for SelfWakePercent {
    fn default() -> Self {
        Self::new(Self::DEFAULT_PERCENT)
    }
}

impl Warning<Task> for SelfWakePercent {
    fn check(&self, task: &Task) -> Option<tui::text::Spans> {
        let self_wakes = task.self_wake_percent();
        if self_wakes > self.min_percent {
            let name = task.name();
            let task = if name.is_empty() {
                Span::from(format!("⚠ Task ID {} ", task.id()))
            } else {
                Span::from(format!("⚠ Task '{}' (ID {}) ", name, task.id()))
            };
            return Some(Spans::from(vec![
                task,
                Span::from(format!(
                    "has woken itself for more than {:.2}% of its total wakeups ({:.2}%)",
                    self.min_percent, self_wakes
                )),
            ]));
        }

        None
    }
}
