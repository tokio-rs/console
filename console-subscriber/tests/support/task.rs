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
    pub fn name(&self) -> &str {
        match &self.match_name {
            Some(name) => name,
            None => "",
        }
    }

    pub fn matches_actual_task(&self, actual_task: &ActualTask) -> bool {
        if let Some(match_name) = &self.match_name {
            if Some(match_name) == actual_task.name.as_ref() {
                return true;
            }
        }

        false
    }

    pub fn validate_actual_task(&self, actual_task: &ActualTask) -> bool {
        let mut no_expectations = true;
        if let Some(expected_wakes) = self.expect_wakes {
            no_expectations = false;
            if expected_wakes != actual_task.wakes {
                println!(
                    "error: Task<name={name}>: expected `wakes` to be {expected_wakes}, but actual was {actual_wakes}",
                    name = self.name(),
                    actual_wakes = actual_task.wakes);
                return false;
            }
        }

        if let Some(expected_self_wakes) = self.expect_self_wakes {
            no_expectations = false;
            if expected_self_wakes != actual_task.self_wakes {
                println!(
                    "error: Task<name={name}>: expected `self_wakes` to be {expected_self_wakes}, but actual was {actual_self_wakes}",
                    name = self.name(),
                    actual_self_wakes = actual_task.self_wakes);
                return false;
            }
        }

        if no_expectations {
            println!(
                "warn: Task<name={name}>: validated, but no expectations found. Did you forget to set some?",
                name = self.name());
        }

        true
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
