use std::{collections::HashMap, fmt, future::Future, thread};

use console_api::{
    field::Value,
    instrument::{instrument_client::InstrumentClient, InstrumentRequest},
};
use console_subscriber::ServerParts;
use futures::stream::StreamExt;
use tokio::{io::DuplexStream, task};
use tonic::transport::{Channel, Endpoint, Server, Uri};
use tower::service_fn;

use super::state::{TestState, TestStep};
use super::task::{ActualTask, ExpectedTask, TaskValidationFailure};

pub(crate) const MAIN_TASK_NAME: &str = "main";

#[derive(Debug)]
struct TestFailure {
    failures: Vec<TaskValidationFailure>,
}

impl fmt::Display for TestFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Task validation failed:\n")?;
        for failure in &self.failures {
            write!(f, " - {failure}\n")?;
        }
        Ok(())
    }
}

#[track_caller]
pub(super) fn run_test<Fut>(expected_tasks: Vec<ExpectedTask>, future: Fut)
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

                let actual_tasks = task::Builder::new()
                    .name("console-client")
                    .spawn(console_client(client_stream, test_state.clone()))
                    .expect("console-test error: could not spawn 'console-client' task")
                    .await
                    .expect("console-test error: failed to await 'console-client' task");

                test_state.advance_to_step(TestStep::Completed);
                actual_tasks
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

    let actual_tasks = join_handle
        .join()
        .expect("console-test error: failed to join 'console-subscriber' thread");

    if let Err(test_failure) = validate_expected_tasks(expected_tasks, actual_tasks) {
        panic!("Test failed: {test_failure}")
    }
}

/// Starts the console server.
///
/// The server will start serving over its side of the duplex stream.
///
/// Once the server gets spawned into its task, the test state is advanced
/// to the `ServerStarted` step. This function will then wait until the test
/// state reaches the `Completed` step (indicating that all validation of the
/// received updates has been completed) before dropping the aggregator.
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

/// Starts the console client and validates the expected tasks.
///
/// First we wait until the server has started (test step `ServerStarted`), then
/// the client is connected to its half of the duplex stream and we start recording
/// the actual tasks.
///
/// Once recording finishes (see [`record_actual_tasks()`] for details on the test
/// state condition), the actual tasks returned.
async fn console_client(client_stream: DuplexStream, mut test_state: TestState) -> Vec<ActualTask> {
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

    record_actual_tasks(channel, test_state.clone()).await
}

/// Records the actual tasks which are received by the client channel.
///
/// Updates will be received until the test state reaches the `TestFinished` step
/// (indicating that the test itself has finished running), at which point we wait
/// for a final update before returning all the actual tasks which were recorded.
async fn record_actual_tasks(
    client_channel: Channel,
    mut test_state: TestState,
) -> Vec<ActualTask> {
    let mut client = InstrumentClient::new(client_channel);

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

/// Validate the expected tasks against the actual tasks.
///
/// Each expected task is checked in turn.
///
/// A matching actual task is searched for. If one is found it, the
/// expected task is validated against the actual task.
///
/// Any validation errors result in failure. If no matches
fn validate_expected_tasks(
    expected_tasks: Vec<ExpectedTask>,
    actual_tasks: Vec<ActualTask>,
) -> Result<(), TestFailure> {
    let failures: Vec<_> = expected_tasks
        .iter()
        .map(|expected| validate_expected_task(expected, &actual_tasks))
        .filter_map(|r| match r {
            Ok(_) => None,
            Err(validation_error) => Some(validation_error),
        })
        .collect();

    if failures.is_empty() {
        Ok(())
    } else {
        Err(TestFailure { failures: failures })
    }
}

fn validate_expected_task(
    expected: &ExpectedTask,
    actual_tasks: &Vec<ActualTask>,
) -> Result<(), TaskValidationFailure> {
    for actual in actual_tasks {
        //println!("actual task: {actual:?}");
        if expected.matches_actual_task(actual) {
            // We only match a single task.
            // FIXME(hds): We should probably create an error or a warning if multiple tasks match.
            return expected.validate_actual_task(actual);
        }
    }

    expected.no_match_error()
}
