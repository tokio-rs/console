use crate::tasks::Task;
use std::{fmt::Debug, rc::Rc};

/// A warning for a particular type of monitored entity (e.g. task or resource).
///
/// This trait implements the logic for detecting a particular warning, and
/// generating a warning message describing it. The [`Linter`] type wraps an
/// instance of this trait to track active instances of the warning.
pub trait Warn<T>: Debug {
    /// Returns `true` if the warning applies to `val`.
    fn check(&self, val: &T) -> bool;

    /// Formats a description of the warning detected for a *specific* `val`.
    ///
    /// This may include dynamically formatted content specific to `val`, such
    /// as the specific numeric value that was over the line for detecting the
    /// warning.
    ///
    /// This should be a complete sentence describing the warning. For example,
    /// for the [`SelfWakePercent`] warning, this returns a string like:
    ///
    /// > "This task has woken itself for more than 50% of its total wakeups (86%)"
    fn format(&self, val: &T) -> String;

    /// Returns a string summarizing the warning *in general*, suitable for
    /// displaying in a list of all detected warnings.
    ///
    /// The list entry will begin with a count of the number of monitored
    /// entities for which the warning was detected. Therefore, this should be a
    /// sentence fragment suitable to follow a count. For example, for the
    /// [`SelfWakePercent`] warning, this method will return a string like
    ///
    /// > "tasks have woken themselves more than 50% of the time"
    ///
    /// so that the warnings list can read
    ///
    /// > "45 tasks have woken themselves more than 50% of the time"
    ///
    /// If the warning is configurable (for example, [`SelfWakePercent`] allows
    /// customizing the percentage used to detect the warning), this string may
    /// be formatted dynamically for the *linter*, but not for the individual
    /// instances of the lint that were detected.
    //
    // TODO(eliza): it would be nice if we had separate plural and singular
    // versions of this, like "56 tasks have..." vs "1 task has...".
    fn summary(&self) -> &str;
}

#[derive(Debug)]
pub(crate) struct Linter<T>(Rc<dyn Warn<T>>);

impl<T> Linter<T> {
    pub(crate) fn new<W>(warning: W) -> Self
    where
        W: Warn<T> + 'static,
    {
        Self(Rc::new(warning))
    }

    /// Checks if the warning applies to a particular entity, returning a clone
    /// of `Self` if it does.
    ///
    /// The cloned instance of `Self` should be held by the entity that
    /// generated the warning, so that it can be formatted. Holding the clone of
    /// `Self` will increment the warning count for that entity.
    pub(crate) fn check(&self, val: &T) -> Option<Self> {
        if self.0.check(val) {
            Some(Self(self.0.clone()))
        } else {
            None
        }
    }

    /// Returns the number of monitored entities that currently have this warning.
    pub(crate) fn count(&self) -> usize {
        Rc::strong_count(&self.0) - 1
    }

    pub(crate) fn format(&self, val: &T) -> String {
        debug_assert!(
            self.0.check(val),
            "tried to format a warning for a {} that did not have that warning!",
            std::any::type_name::<T>()
        );
        self.0.format(val)
    }

    pub(crate) fn summary(&self) -> &str {
        self.0.summary()
    }
}

#[derive(Clone, Debug)]
pub(crate) struct SelfWakePercent {
    min_percent: u64,
    description: String,
}

impl SelfWakePercent {
    pub(crate) const DEFAULT_PERCENT: u64 = 50;
    pub(crate) fn new(min_percent: u64) -> Self {
        Self {
            min_percent,
            description: format!(
                "tasks have woken themselves over {}% of the time",
                min_percent
            ),
        }
    }
}

impl Default for SelfWakePercent {
    fn default() -> Self {
        Self::new(Self::DEFAULT_PERCENT)
    }
}

impl Warn<Task> for SelfWakePercent {
    fn summary(&self) -> &str {
        self.description.as_str()
    }

    fn check(&self, task: &Task) -> bool {
        let self_wakes = task.self_wake_percent();
        self_wakes > self.min_percent
    }

    fn format(&self, task: &Task) -> String {
        let self_wakes = task.self_wake_percent();
        format!(
            "This task has woken itself for more than {}% of its total wakeups ({}%)",
            self.min_percent, self_wakes
        )
    }
}
