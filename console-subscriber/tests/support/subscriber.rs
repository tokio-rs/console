use std::{collections::HashMap, fmt, future::Future, thread};

use console_api::{
    field::Value,
    instrument::{instrument_client::InstrumentClient, InstrumentRequest},
};
use console_subscriber::ServerParts;
use futures::stream::StreamExt;
use hyper_util::rt::TokioIo;
use tokio::{io::DuplexStream, task};
use tonic::transport::{Channel, Endpoint, Server, Uri};
use tower::service_fn;

use super::state::{TestState, TestStep};
use super::task::{ActualTask, ExpectedTask, TaskValidationFailure};

pub(crate) const MAIN_TASK_NAME: &str = "console-test::main";
const END_SIGNAL_TASK_NAME: &str = "console-test::signal";

#[derive(Debug)]
struct TestFailure {
    failures: Vec<TaskValidationFailure>,
}

impl fmt::Display for TestFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "Task validation failed:")?;
        for failure in &self.failures {
            writeln!(f, " - {failure}")?;
        }
        Ok(())
    }
}

/// Runs the test
///
/// This function runs the whole test. It sets up a `console-subscriber` layer
/// together with the gRPC server and connects a client to it. The subscriber
/// is then used to record traces as the provided future is driven to
/// completion on a current thread tokio runtime.
///
/// This function will panic if the expectations on any of the expected tasks
/// are not met or if matching tasks are not recorded for all expected tasks.
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

    let thread_name = {
        // Include the name of the test thread in the spawned subscriber thread,
        // to make it clearer which test it belongs to.
        let current_thread = thread::current();
        let test = current_thread.name().unwrap_or("<unknown test>");
        format!("{test}-console::subscriber")
    };
    let join_handle = thread::Builder::new()
        .name(thread_name)
        // Run the test's console server and client tasks in a separate thread
        // from the main test, ensuring that any `tracing` emitted by the
        // console worker and the client are not collected by the subscriber
        // under test.
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
                    .name("console::serve")
                    .spawn(console_server(server, server_stream, test_state.clone()))
                    .expect("console-test error: could not spawn 'console-server' task");

                let actual_tasks = task::Builder::new()
                    .name("console::client")
                    .spawn(console_client(client_stream, test_state.clone()))
                    .expect("console-test error: could not spawn 'console-client' task")
                    .await
                    .expect("console-test error: failed to await 'console-client' task");

                test_state.advance_to_step(TestStep::UpdatesRecorded);
                actual_tasks
            })
        })
        .expect("console-test error: console subscriber could not spawn thread");

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
            _ = task::Builder::new()
                .name(MAIN_TASK_NAME)
                .spawn(future)
                .expect("console-test error: couldn't spawn test task")
                .await;
            _ = task::Builder::new()
                .name(END_SIGNAL_TASK_NAME)
                .spawn(futures::future::ready(()))
                .expect("console-test error: couldn't spawn end signal task")
                .await;
            test_state_test.advance_to_step(TestStep::TestFinished);

            test_state_test
                .wait_for_step(TestStep::UpdatesRecorded)
                .await;
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
/// state reaches the `UpdatesRecorded` step (indicating that all validation of the
/// received updates has been completed) before dropping the aggregator.
///
/// # Test State
///
/// 1. Advances to: `ServerStarted`
/// 2. Waits for: `UpdatesRecorded`
async fn console_server(
    server: console_subscriber::Server,
    server_stream: DuplexStream,
    mut test_state: TestState,
) {
    let ServerParts {
        instrument_server: service,
        aggregator,
        ..
    } = server.into_parts();
    let aggregate = task::Builder::new()
        .name("console::aggregate")
        .spawn(aggregator.run())
        .expect("console-test error: couldn't spawn aggregator");
    Server::builder()
        .add_service(service)
        // .serve_with_incoming(futures::stream::once(Ok::<_, std::io::Error>(server_stream)))
        .serve_with_incoming(futures::stream::iter(vec![Ok::<_, std::io::Error>(
            server_stream,
        )]))
        .await
        .expect("console-test error: couldn't start instrument server.");
    test_state.advance_to_step(TestStep::ServerStarted);

    test_state.wait_for_step(TestStep::UpdatesRecorded).await;
    aggregate.abort();
}

/// Starts the console client and validates the expected tasks.
///
/// First we wait until the server has started (test step `ServerStarted`), then
/// the client is connected to its half of the duplex stream and we start recording
/// the actual tasks.
///
/// Once recording finishes (see [`record_actual_tasks()`] for details on the test
/// state condition), the actual tasks returned.
///
/// # Test State
///
/// 1. Waits for: `ServerStarted`
/// 2. Advances to: `ClientConnected`
async fn console_client(client_stream: DuplexStream, mut test_state: TestState) -> Vec<ActualTask> {
    test_state.wait_for_step(TestStep::ServerStarted).await;

    let mut client_stream = Some(client_stream);
    // Note: we won't actually try to connect to this port on localhost,
    // because we will call `connect_with_connector` with a service that
    // just returns the `DuplexStream`, instead of making an actual
    // network connection.
    let endpoint = Endpoint::try_from("http://[::]:6669").expect("Could not create endpoint");
    let channel = endpoint
        .connect_with_connector(service_fn(move |_: Uri| {
            let client = client_stream.take();

            async move {
                // We need to return a Result from this async block, which is
                // why we don't unwrap the `client` here.
                client.map(TokioIo::new).ok_or_else(|| {
                    std::io::Error::other(
                        "console-test error: client already taken. This shouldn't happen.",
                    )
                })
            }
        }))
        .await
        .expect("console-test client error: couldn't create client");
    test_state.advance_to_step(TestStep::ClientConnected);

    record_actual_tasks(channel, test_state).await
}

/// Records the actual tasks which are received by the client channel.
///
/// Updates will be received until the test state reaches the `TestFinished` step
/// (indicating that the test itself has finished running), at which point we wait
/// for a final update before returning all the actual tasks which were recorded.
///
/// # Test State
///
/// 1. Waits for: `TestFinished`
async fn record_actual_tasks(
    client_channel: Channel,
    mut test_state: TestState,
) -> Vec<ActualTask> {
    let mut client = InstrumentClient::new(client_channel);

    let mut stream = match client
        .watch_updates(tonic::Request::new(InstrumentRequest {}))
        .await
    {
        Ok(stream) => stream.into_inner(),
        Err(err) => panic!("console-test error: client cannot connect to watch updates: {err}"),
    };

    let mut tasks = HashMap::new();

    // The console-subscriber aggregator is a bit of an unknown entity for us,
    // especially with respect to its update loops. We can't guarantee that
    // it will have seen all the tasks in our test N iterations after the test
    // ends for some known N. For this reason we need to use a signal task to
    // check for and end the collection of events at that point.
    let signal_task = ExpectedTask::default().match_name(END_SIGNAL_TASK_NAME.into());
    let mut signal_task_read = false;
    while let Some(update) = stream.next().await {
        let update = update.expect("console-test error: update stream error");

        if let Some(task_update) = &update.task_update {
            for new_task in &task_update.new_tasks {
                let mut actual_task = match new_task.id {
                    Some(id) => ActualTask::new(id.id),
                    None => continue,
                };
                for field in &new_task.fields {
                    match field.name.as_ref() {
                        Some(console_api::field::Name::StrName(name)) if name == "task.name" => {
                            actual_task.name = match field.value.as_ref() {
                                Some(Value::DebugVal(value)) => Some(value.clone()),
                                Some(Value::StrVal(value)) => Some(value.clone()),
                                _ => continue,
                            };
                        }
                        _ => {}
                    }
                }

                if signal_task.matches_actual_task(&actual_task) {
                    signal_task_read = true;
                } else {
                    tasks.insert(actual_task.id, actual_task);
                }
            }

            for (id, stats) in &task_update.stats_update {
                if let Some(task) = tasks.get_mut(id) {
                    task.update_from_stats(stats);
                }
            }
        }

        if test_state.is_step(TestStep::TestFinished) && signal_task_read {
            // Once the test finishes running and we've read the signal task, the test ends.
            break;
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
        .filter_map(Result::err)
        .collect();

    if failures.is_empty() {
        Ok(())
    } else {
        Err(TestFailure { failures })
    }
}

#[allow(clippy::result_large_err)]
fn validate_expected_task(
    expected: &ExpectedTask,
    actual_tasks: &Vec<ActualTask>,
) -> Result<(), TaskValidationFailure> {
    for actual in actual_tasks {
        if expected.matches_actual_task(actual) {
            // We only match a single task.
            // FIXME(hds): We should probably create an error or a warning if multiple tasks match.
            return expected.validate_actual_task(actual);
        }
    }

    expected.no_match_error()
}
