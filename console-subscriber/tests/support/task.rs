use std::{error, fmt};

use console_api::tasks;

use super::MAIN_TASK_NAME;

/// An actual task
///
/// This struct contains the values recorded from the console subscriber
/// client and represents what is known about an actual task running on
/// the test's runtime.
#[derive(Clone, Debug)]
pub(super) struct ActualTask {
    pub(super) id: u64,
    pub(super) name: Option<String>,
    pub(super) wakes: u64,
    pub(super) self_wakes: u64,
    pub(super) polls: u64,
}

impl ActualTask {
    pub(super) fn new(id: u64) -> Self {
        Self {
            id,
            name: None,
            wakes: 0,
            self_wakes: 0,
            polls: 0,
        }
    }

    pub(super) fn update_from_stats(&mut self, stats: &tasks::Stats) {
        self.wakes = stats.wakes;
        self.self_wakes = stats.self_wakes;
        if let Some(poll_stats) = &stats.poll_stats {
            self.polls = poll_stats.polls;
        }
    }
}

/// An error in task validation.
pub(super) struct TaskValidationFailure {
    /// The expected task whose expectations were not met.
    expected: ExpectedTask,
    /// The actual task which failed the validation
    actual: Option<ActualTask>,
    /// A textual description of the validation failure
    failure: String,
}

impl error::Error for TaskValidationFailure {}

impl fmt::Display for TaskValidationFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.failure)
    }
}

impl fmt::Debug for TaskValidationFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.actual {
            Some(actual) => write!(
                f,
                "Task Validation Failed!\n  Expected Task: {expected:?}\
                \n  Actual Task:   {actual:?}\
                \n  Failure:       {failure}",
                expected = self.expected,
                failure = self.failure,
            ),
            None => write!(
                f,
                "Task Validation Failed!\n  Expected Task: {expected:?}\
                \n  Actual Task:   <not found>\
                \n  Failure:       {failure}",
                expected = self.expected,
                failure = self.failure,
            ),
        }
    }
}

/// An expected task.
///
/// This struct contains the fields that an expected task will attempt to match
/// actual tasks on, as well as the expectations that will be used to validate
/// which the actual task is as expected.
#[derive(Clone, Debug, Default)]
pub(crate) struct ExpectedTask {
    match_name: Option<String>,
    expect_present: Option<bool>,
    expect_wakes: Option<u64>,
    expect_self_wakes: Option<u64>,
    expect_polls: Option<u64>,
}

impl ExpectedTask {
    /// Returns whether or not an actual task matches this expected task.
    ///
    /// All matching rules will be run, if they all succeed, then `true` will
    /// be returned, otherwise `false`.
    pub(super) fn matches_actual_task(&self, actual_task: &ActualTask) -> bool {
        if let Some(match_name) = &self.match_name {
            if Some(match_name) == actual_task.name.as_ref() {
                return true;
            }
        }

        false
    }

    /// Returns an error specifying that no match was found for this expected
    /// task.
    #[allow(clippy::result_large_err)]
    pub(super) fn no_match_error(&self) -> Result<(), TaskValidationFailure> {
        Err(TaskValidationFailure {
            expected: self.clone(),
            actual: None,
            failure: format!("{self}: no matching actual task was found"),
        })
    }

    /// Validates all expectations against the provided actual task.
    ///
    /// No check that the actual task matches is performed. That must have been
    /// done prior.
    ///
    /// If all expectations are met, this method returns `Ok(())`. If any
    /// expectations are not met, then the first incorrect expectation will
    /// be returned as an `Err`.
    #[allow(clippy::result_large_err)]
    pub(super) fn validate_actual_task(
        &self,
        actual_task: &ActualTask,
    ) -> Result<(), TaskValidationFailure> {
        let mut no_expectations = true;
        if let Some(_expected) = self.expect_present {
            no_expectations = false;
        }

        if let Some(expected_wakes) = self.expect_wakes {
            no_expectations = false;
            if expected_wakes != actual_task.wakes {
                return Err(TaskValidationFailure {
                    expected: self.clone(),
                    actual: Some(actual_task.clone()),
                    failure: format!(
                        "{self}: expected `wakes` to be {expected_wakes}, but \
                        actual was {actual_wakes}",
                        actual_wakes = actual_task.wakes,
                    ),
                });
            }
        }

        if let Some(expected_self_wakes) = self.expect_self_wakes {
            no_expectations = false;
            if expected_self_wakes != actual_task.self_wakes {
                return Err(TaskValidationFailure {
                    expected: self.clone(),
                    actual: Some(actual_task.clone()),
                    failure: format!(
                        "{self}: expected `self_wakes` to be \
                        {expected_self_wakes}, but actual was \
                        {actual_self_wakes}",
                        actual_self_wakes = actual_task.self_wakes,
                    ),
                });
            }
        }

        if let Some(expected_polls) = self.expect_polls {
            no_expectations = false;
            if expected_polls != actual_task.polls {
                return Err(TaskValidationFailure {
                    expected: self.clone(),
                    actual: Some(actual_task.clone()),
                    failure: format!(
                        "{self}: expected `polls` to be {expected_polls}, but \
                        actual was {actual_polls}",
                        actual_polls = actual_task.polls,
                    ),
                });
            }
        }

        if no_expectations {
            return Err(TaskValidationFailure {
                expected: self.clone(),
                actual: Some(actual_task.clone()),
                failure: format!(
                    "{self}: no expectations set, if you want to just expect \
                    that a matching task is present, use `expect_present()`",
                ),
            });
        }

        Ok(())
    }

    /// Matches tasks by name.
    ///
    /// To match this expected task, an actual task must have the name `name`.
    #[allow(dead_code)]
    pub(crate) fn match_name(mut self, name: String) -> Self {
        self.match_name = Some(name);
        self
    }

    /// Matches tasks by the default task name.
    ///
    /// To match this expected task, an actual task must have the default name
    /// assigned to the task which runs the future provided to [`assert_task`]
    /// or [`assert_tasks`].
    ///
    /// [`assert_task`]: fn@support::assert_task
    /// [`assert_tasks`]: fn@support::assert_tasks
    #[allow(dead_code)]
    pub(crate) fn match_default_name(mut self) -> Self {
        self.match_name = Some(MAIN_TASK_NAME.into());
        self
    }

    /// Expects that a task is present.
    ///
    /// To validate, an actual task matching this expected task must be found.
    #[allow(dead_code)]
    pub(crate) fn expect_present(mut self) -> Self {
        self.expect_present = Some(true);
        self
    }

    /// Expects that a task has a specific value for `wakes`.
    ///
    /// To validate, the actual task matching this expected task must have
    /// a count of wakes equal to `wakes`.
    #[allow(dead_code)]
    pub(crate) fn expect_wakes(mut self, wakes: u64) -> Self {
        self.expect_wakes = Some(wakes);
        self
    }

    /// Expects that a task has a specific value for `self_wakes`.
    ///
    /// To validate, the actual task matching this expected task must have
    /// a count of self wakes equal to `self_wakes`.
    #[allow(dead_code)]
    pub(crate) fn expect_self_wakes(mut self, self_wakes: u64) -> Self {
        self.expect_self_wakes = Some(self_wakes);
        self
    }

    /// Expects that a task has a specific value for `polls`.
    ///
    /// To validate, the actual task must have a count of polls (on
    /// `PollStats`) equal to `polls`.
    #[allow(dead_code)]
    pub(crate) fn expect_polls(mut self, polls: u64) -> Self {
        self.expect_polls = Some(polls);
        self
    }
}

impl fmt::Display for ExpectedTask {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let fields = match &self.match_name {
            Some(name) => format!("name={name}"),
            None => "(no fields to match on)".into(),
        };
        write!(f, "Task {{ {fields} }}")
    }
}
