use std::{error, fmt};

use super::MAIN_TASK_NAME;

#[derive(Clone, Debug)]
pub(super) struct ActualTask {
    pub(super) id: u64,
    pub(super) name: Option<String>,
    pub(super) wakes: u64,
    pub(super) self_wakes: u64,
}

impl ActualTask {
    pub(super) fn new(id: u64) -> Self {
        Self {
            id,
            name: None,
            wakes: 0,
            self_wakes: 0,
        }
    }
}

pub(super) struct TaskValidationFailure {
    expected: ExpectedTask,
    actual: Option<ActualTask>,
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
                "Task Validation Failed!\n  Expected Task: {expected:?}\n  Actual Task:   {actual:?}\n  Failure:       {failure}",
                expected = self.expected, failure = self.failure),
            None => write!(
                f,
                "Task Validation Failed!\n  Expected Task: {expected:?}\n  Failure:       {failure}",
                expected = self.expected, failure = self.failure),
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct ExpectedTask {
    match_name: Option<String>,

    expect_present: Option<bool>,
    expect_wakes: Option<u64>,
    expect_self_wakes: Option<u64>,
}

impl Default for ExpectedTask {
    fn default() -> Self {
        Self {
            match_name: None,
            expect_present: None,
            expect_wakes: None,
            expect_self_wakes: None,
        }
    }
}

impl ExpectedTask {
    pub(super) fn matches_actual_task(&self, actual_task: &ActualTask) -> bool {
        if let Some(match_name) = &self.match_name {
            if Some(match_name) == actual_task.name.as_ref() {
                return true;
            }
        }

        false
    }

    pub(super) fn no_match_error(&self) -> Result<(), TaskValidationFailure> {
        Err(TaskValidationFailure {
            expected: self.clone(),
            actual: None,
            failure: format!("{self}: no matching actual task was found"),
        })
    }

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
                        "{self}: expected `wakes` to be {expected_wakes}, but actual was {actual_wakes}",
                        actual_wakes = actual_task.wakes),
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
                        "{self}: expected `self_wakes` to be {expected_self_wakes}, but actual was {actual_self_wakes}",
                        actual_self_wakes = actual_task.self_wakes),
                });
            }
        }

        if no_expectations {
            return Err(TaskValidationFailure {
                expected: self.clone(),
                actual: Some(actual_task.clone()),
                failure: format!(
                    "{self}: no expectations set, if you want to just expect that a matching task is present, use `expect_present()`")
            });
        }

        Ok(())
    }

    #[allow(dead_code)]
    pub(crate) fn match_name(mut self, name: String) -> Self {
        self.match_name = Some(name);
        self
    }

    #[allow(dead_code)]
    pub(crate) fn match_default_name(mut self) -> Self {
        self.match_name = Some(MAIN_TASK_NAME.into());
        self
    }

    #[allow(dead_code)]
    pub(crate) fn expect_present(mut self) -> Self {
        self.expect_present = Some(true);
        self
    }

    #[allow(dead_code)]
    pub(crate) fn expect_wakes(mut self, wakes: u64) -> Self {
        self.expect_wakes = Some(wakes);
        self
    }

    #[allow(dead_code)]
    pub(crate) fn expect_self_wakes(mut self, self_wakes: u64) -> Self {
        self.expect_self_wakes = Some(self_wakes);
        self
    }
}

impl fmt::Display for ExpectedTask {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let fields = match &self.match_name {
            Some(name) => format!("name={name}"),
            None => "(no fields to match on)".into(),
        };
        write!(f, "Task<{fields}>")
    }
}
