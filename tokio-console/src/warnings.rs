use crate::state::tasks::{Task, TaskState};
use std::{
    fmt::Debug,
    rc::Rc,
    time::{Duration, SystemTime},
};

/// A warning for a particular type of monitored entity (e.g. task or resource).
///
/// This trait implements the logic for detecting a particular warning, and
/// generating a warning message describing it. The [`Linter`] type wraps an
/// instance of this trait to track active instances of the warning.
pub trait Warn<T>: Debug {
    /// Returns if the warning applies to `val`.
    fn check(&self, val: &T) -> Warning;

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

/// A result for a linter check
pub(crate) enum Lint<T> {
    /// No warning applies to the entity
    Ok,

    /// The cloned instance of `Self` should be held by the entity that
    /// generated the warning, so that it can be formatted. Holding the clone of
    /// `Self` will increment the warning count for that entity.
    Warning(Linter<T>),

    /// The lint should be rechecked as the conditions to allow for checking are
    /// not satisfied yet
    Recheck,
}

/// A result for a warning check
pub enum Warning {
    /// Set `true` if the warning applies or `false` otherwise
    Check(bool),

    /// The warning should be rechecked as the conditions to allow for checking
    /// are not satisfied yet
    Recheck,
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

    /// Checks if the warning applies to a particular entity
    pub(crate) fn check(&self, val: &T) -> Lint<T> {
        match self.0.check(val) {
            Warning::Check(false) => Lint::Ok,
            Warning::Check(true) => Lint::Warning(Self(self.0.clone())),
            Warning::Recheck => Lint::Recheck,
        }
    }

    /// Returns the number of monitored entities that currently have this warning.
    pub(crate) fn count(&self) -> usize {
        Rc::strong_count(&self.0) - 1
    }

    pub(crate) fn format(&self, val: &T) -> String {
        debug_assert!(
            matches!(self.0.check(val), Warning::Check(true)),
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

    fn check(&self, task: &Task) -> Warning {
        let self_wakes = task.self_wake_percent();
        Warning::Check(self_wakes > self.min_percent)
    }

    fn format(&self, task: &Task) -> String {
        let self_wakes = task.self_wake_percent();
        format!(
            "This task has woken itself for more than {}% of its total wakeups ({}%)",
            self.min_percent, self_wakes
        )
    }
}

#[derive(Clone, Debug, Default)]
pub(crate) struct LostWaker;

impl Warn<Task> for LostWaker {
    fn summary(&self) -> &str {
        "tasks have lost their waker"
    }

    fn check(&self, task: &Task) -> Warning {
        Warning::Check(
            !task.is_completed()
                && task.waker_count() == 0
                && !task.is_running()
                && !task.is_awakened(),
        )
    }

    fn format(&self, _: &Task) -> String {
        "This task has lost its waker, and will never be woken again.".into()
    }
}

/// Warning for if a task has never yielded
#[derive(Clone, Debug)]
pub(crate) struct NeverYielded {
    min_duration: Duration,
    description: String,
}

impl NeverYielded {
    pub(crate) const DEFAULT_DURATION: Duration = Duration::from_secs(1);
    pub(crate) fn new(min_duration: Duration) -> Self {
        Self {
            min_duration,
            description: format!(
                "tasks have never yielded (threshold {}ms)",
                min_duration.as_millis()
            ),
        }
    }
}

impl Default for NeverYielded {
    fn default() -> Self {
        Self::new(Self::DEFAULT_DURATION)
    }
}

impl Warn<Task> for NeverYielded {
    fn summary(&self) -> &str {
        self.description.as_str()
    }

    fn check(&self, task: &Task) -> Warning {
        // Don't fire warning for tasks that are waiting to run
        if task.state() != TaskState::Running {
            return Warning::Check(false);
        }

        if task.total_polls() > 1 {
            return Warning::Check(false);
        }

        // Avoid short-lived task false positives
        if task.busy(SystemTime::now()) >= self.min_duration {
            Warning::Check(true)
        } else {
            Warning::Recheck
        }
    }

    fn format(&self, task: &Task) -> String {
        format!(
            "This task has never yielded ({:?})",
            task.busy(SystemTime::now()),
        )
    }
}
