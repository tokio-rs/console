use std::{collections::HashMap, fmt, future::Future, thread};

use console_api::{
    field::Value,
    instrument::{instrument_client::InstrumentClient, InstrumentRequest},
};
use console_subscriber::ServerParts;
use futures::stream::StreamExt;
use tokio::{
    io::DuplexStream,
    sync::broadcast::{
        self,
        error::{RecvError, TryRecvError},
    },
    task,
};
use tonic::transport::{Channel, Endpoint, Server, Uri};
use tower::service_fn;

use super::task::{ActualTask, ExpectedTask, TaskValidationFailure};

pub const MAIN_TASK_NAME: &str = "main";

#[derive(Debug)]
enum TestFailure {
    NoTasksMatched,
    TasksFailedValidation {
        failures: Vec<TaskValidationFailure>,
    },
}

impl fmt::Display for TestFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoTasksMatched => write!(f, "No tasks matched the expected tasks."),
            Self::TasksFailedValidation { failures } => {
                write!(f, "Task validation failed:\n")?;
                for failure in failures {
                    write!(f, " - {failure}\n")?;
                }
                Ok(())
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq, PartialOrd)]
enum TestStep {
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

struct TestState {
    receiver: broadcast::Receiver<TestStep>,
    sender: broadcast::Sender<TestStep>,
    step: TestStep,
}

impl TestState {
    fn new() -> Self {
        let (sender, receiver) = broadcast::channel(1);
        Self {
            receiver,
            sender,
            step: TestStep::Start,
        }
    }

    async fn wait_for_step(&mut self, desired_step: TestStep) {
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

    fn try_wait_for_step(&mut self, desired_step: TestStep) -> bool {
        self.update_step();

        self.step == desired_step
    }

    #[track_caller]
    fn advance_to_step(&mut self, next_step: TestStep) {
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

#[track_caller]
pub fn assert_tasks<Fut>(expected_tasks: Vec<ExpectedTask>, future: Fut)
where
    Fut: Future + Send + 'static,
    Fut::Output: Send + 'static,
{
    use tracing_subscriber::prelude::*;

    let (client_stream, server_stream) = tokio::io::duplex(1024);
    let (console_layer, server) = console_subscriber::ConsoleLayer::builder().build();
    let registry = tracing_subscriber::registry().with(console_layer);

    let mut test_state = TestState::new();
    let mut test_state_test = test_state.clone();

    let join_handle = thread::Builder::new()
        .name("console-subscriber".into())
        .spawn(move || {
            let _subscriber_guard =
                tracing::subscriber::set_default(tracing_core::subscriber::NoSubscriber::default());
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_io()
                .enable_time()
                .build()
                .expect("console-test error: failed to initialize console subscriber runtime");

            runtime.block_on(async move {
                task::Builder::new()
                    .name("console-server")
                    .spawn(console_server(server, server_stream, test_state.clone()))
                    .expect("console-test error: could not spawn 'console-server' task");

                let console_client = task::Builder::new()
                    .name("console-client")
                    .spawn(async move {
                        test_state.wait_for_step(TestStep::ServerStarted).await;

                        let mut client_stream = Some(client_stream);
                        let channel = Endpoint::try_from("http://[::]:6669")
                            .expect("Could not create endpoint")
                            .connect_with_connector(service_fn(move |_: Uri| {
                                let client = client_stream.take();

                                async move {
                                    if let Some(client) = client {
                                        Ok(client)
                                    } else {
                                        Err(std::io::Error::new(
                                            std::io::ErrorKind::Other,
                                            "Client already taken",
                                        ))
                                    }
                                }
                            }))
                            .await
                            .expect("client-console error: couldn't create client");
                        test_state.advance_to_step(TestStep::ClientConnected);

                        let actual_tasks = record_actual_tasks(channel, test_state.clone()).await;

                        let mut validation_results = Vec::new();
                        for expected in &expected_tasks {
                            for actual in &actual_tasks {
                                if expected.matches_actual_task(actual) {
                                    validation_results.push(expected.validate_actual_task(actual));

                                    // We only match a single task.
                                    // FIXME(hds): We should probably create an error or a warning if multiple tasks match.
                                    continue;
                                }
                            }
                        }

                        test_state.advance_to_step(TestStep::Completed);

                        test_result(validation_results)
                    })
                    .expect("console-test error: could not spawn 'console-client' task");

                console_client
                    .await
                    .expect("console-test error: failed to await 'console-client' task")
            })
        })
        .expect("console subscriber could not spawn thread");

    tracing::subscriber::with_default(registry, || {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();

        runtime.block_on(async move {
            test_state_test
                .wait_for_step(TestStep::ClientConnected)
                .await;

            // Run the future that we are testing.
            _ = tokio::task::Builder::new()
                .name(MAIN_TASK_NAME)
                .spawn(future)
                .expect("console-test error: couldn't spawn test task")
                .await;
            test_state_test.advance_to_step(TestStep::TestFinished);

            test_state_test.wait_for_step(TestStep::Completed).await;
        });
    });

    let test_result = join_handle
        .join()
        .expect("console-test error: failed to join 'console-subscriber' thread");

    if let Err(test_failure) = test_result {
        panic!("Test failed: {test_failure}")
    }
}

async fn console_server(
    server: console_subscriber::Server,
    server_stream: DuplexStream,
    mut test_state: TestState,
) {
    let ServerParts {
        instrument_server: service,
        aggregator_handle: aggregate,
        ..
    } = server.into_parts();
    Server::builder()
        .add_service(service)
        .serve_with_incoming(futures::stream::iter(vec![Ok::<_, std::io::Error>(
            server_stream,
        )]))
        .await
        .expect("console subscriber failed.");
    test_state.advance_to_step(TestStep::ServerStarted);

    test_state.wait_for_step(TestStep::Completed).await;
    drop(aggregate);
}

async fn record_actual_tasks(channel: Channel, mut test_state: TestState) -> Vec<ActualTask> {
    let mut client = InstrumentClient::new(channel);

    let mut stream = loop {
        let request = tonic::Request::new(InstrumentRequest {});
        match client.watch_updates(request).await {
            Ok(stream) => break stream.into_inner(),
            Err(err) => panic!("Client cannot connect to watch updates: {err}"),
        }
    };

    let mut tasks = HashMap::new();

    let mut last_update = false;
    while let Some(update) = stream.next().await {
        match update {
            Ok(update) => {
                if let Some(task_update) = &update.task_update {
                    for new_task in &task_update.new_tasks {
                        if let Some(id) = &new_task.id {
                            let mut actual_task = ActualTask::new(id.id);
                            for field in &new_task.fields {
                                if let Some(console_api::field::Name::StrName(field_name)) =
                                    &field.name
                                {
                                    if field_name == "task.name" {
                                        actual_task.name = match &field.value {
                                            Some(Value::DebugVal(value)) => Some(value.clone()),
                                            Some(Value::StrVal(value)) => Some(value.clone()),
                                            _ => None, // Anything that isn't string-like shouldn't be used as a name.
                                        };
                                    }
                                }
                            }
                            tasks.insert(actual_task.id, actual_task);
                        }
                    }

                    for (id, stats) in &task_update.stats_update {
                        if let Some(mut task) = tasks.get_mut(id) {
                            task.wakes = stats.wakes;
                            task.self_wakes = stats.self_wakes;
                        }
                    }
                }
            }
            Err(e) => {
                panic!("update stream error: {}", e);
            }
        }

        if last_update {
            break;
        }

        if test_state.try_wait_for_step(TestStep::TestFinished) {
            // Once the test finishes running, we will get one further update and finish.
            last_update = true;
        }
    }

    tasks.into_values().collect()
}

fn test_result(
    validation_results: Vec<Result<(), TaskValidationFailure>>,
) -> Result<(), TestFailure> {
    if validation_results.is_empty() {
        return Err(TestFailure::NoTasksMatched);
    }

    let failures: Vec<_> = validation_results
        .into_iter()
        .filter_map(|r| match r {
            Ok(_) => None,
            Err(validation_error) => Some(validation_error),
        })
        .collect();

    if failures.is_empty() {
        Ok(())
    } else {
        Err(TestFailure::TasksFailedValidation { failures: failures })
    }
}
