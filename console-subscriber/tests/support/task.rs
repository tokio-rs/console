use std::{error, fmt};

#[derive(Clone, Debug)]
pub struct ActualTask {
    pub id: u64,
    pub name: Option<String>,
    pub wakes: u64,
    pub self_wakes: u64,
}

impl ActualTask {
    pub fn new(id: u64) -> Self {
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
    actual: ActualTask,
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
        write!(
            f,
            "Task Validation Failed!\n  Expected Task: {expected:?}\n  Actual Task:   {actual:?}\n  Failure:       {failure}",
            expected = self.expected, actual = self.actual, failure = self.failure)
    }
}

#[derive(Clone, Debug)]
pub struct ExpectedTask {
    match_name: Option<String>,

    expect_wakes: Option<u64>,
    expect_self_wakes: Option<u64>,
}

impl Default for ExpectedTask {
    fn default() -> Self {
        Self {
            match_name: Default::default(),
            expect_wakes: Default::default(),
            expect_self_wakes: Default::default(),
        }
    }
}

impl ExpectedTask {
    pub fn matches_actual_task(&self, actual_task: &ActualTask) -> bool {
        if let Some(match_name) = &self.match_name {
            if Some(match_name) == actual_task.name.as_ref() {
                return true;
            }
        }

        false
    }

    pub(super) fn validate_actual_task(
        &self,
        actual_task: &ActualTask,
    ) -> Result<(), TaskValidationFailure> {
        let mut no_expectations = true;
        if let Some(expected_wakes) = self.expect_wakes {
            no_expectations = false;
            if expected_wakes != actual_task.wakes {
                return Err(TaskValidationFailure {
                    expected: self.clone(),
                    actual: actual_task.clone(),
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
                    actual: actual_task.clone(),
                    failure: format!(
                        "{self}: expected `self_wakes` to be {expected_self_wakes}, but actual was {actual_self_wakes}",
                        actual_self_wakes = actual_task.self_wakes),
                });
            }
        }

        if no_expectations {
            println!("{self}: validated, but no expectations found. Did you forget to set some?",);
        }

        Ok(())
    }

    pub fn match_name(mut self, name: String) -> Self {
        self.match_name = Some(name);
        self
    }

    pub fn expect_wakes(mut self, wakes: u64) -> Self {
        self.expect_wakes = Some(wakes);
        self
    }

    pub fn expect_self_wakes(mut self, self_wakes: u64) -> Self {
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
