use std::fmt;

use tokio::sync::broadcast::{
    self,
    error::{RecvError, TryRecvError},
};

#[derive(Clone, Debug, PartialEq, PartialOrd)]
pub(super) enum TestStep {
    Start,
    ServerStarted,
    ClientConnected,
    TestFinished,
    Completed,
}

impl fmt::Display for TestStep {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        (self as &dyn fmt::Debug).fmt(f)
    }
}

pub(super) struct TestState {
    receiver: broadcast::Receiver<TestStep>,
    sender: broadcast::Sender<TestStep>,
    step: TestStep,
}

impl TestState {
    pub(super) fn new() -> Self {
        let (sender, receiver) = broadcast::channel(1);
        Self {
            receiver,
            sender,
            step: TestStep::Start,
        }
    }

    pub(super) async fn wait_for_step(&mut self, desired_step: TestStep) {
        loop {
            if self.step >= desired_step {
                break;
            }

            match self.receiver.recv().await {
                Ok(step) => self.step = step,
                Err(RecvError::Lagged(_)) => {
                    // we don't mind being lagged, we'll just get the latest state
                }
                Err(RecvError::Closed) => {
                    panic!("failed to receive current step, waiting for step: {desired_step}, did the test abort?");
                }
            }
        }
    }

    pub(super) fn try_wait_for_step(&mut self, desired_step: TestStep) -> bool {
        self.update_step();

        self.step == desired_step
    }

    #[track_caller]
    pub(super) fn advance_to_step(&mut self, next_step: TestStep) {
        self.update_step();

        if self.step >= next_step {
            panic!(
                "cannot advance to previous or current step! current step: {current}, next step: {next_step}",
                current = self.step);
        }

        match (&self.step, &next_step) {
            (TestStep::Start, TestStep::ServerStarted) |
            (TestStep::ServerStarted, TestStep::ClientConnected) |
            (TestStep::ClientConnected, TestStep::TestFinished) |
            (TestStep::TestFinished, TestStep::Completed) => {},
            (_, _) => panic!(
                "cannot advance more than one step! current step: {current}, next step: {next_step}",
                current = self.step),
        }

        self.sender
            .send(next_step)
            .expect("failed to send the next test step, did the test abort?");
    }

    fn update_step(&mut self) {
        loop {
            match self.receiver.try_recv() {
                Ok(step) => self.step = step,
                Err(TryRecvError::Lagged(_)) => {
                    // we don't mind being lagged, we'll just get the latest state
                }
                Err(TryRecvError::Closed) => {
                    panic!("failed to update current step, did the test abort?")
                }
                Err(TryRecvError::Empty) => break,
            }
        }
    }
}

impl Clone for TestState {
    fn clone(&self) -> Self {
        Self {
            receiver: self.receiver.resubscribe(),
            sender: self.sender.clone(),
            step: self.step.clone(),
        }
    }
}
