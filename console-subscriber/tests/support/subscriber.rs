use std::{collections::HashMap, fmt, future::Future, thread};

use console_api::{
    field::Value,
    instrument::{instrument_client::InstrumentClient, InstrumentRequest},
};
use futures::stream::StreamExt;
use tokio::{
    io::DuplexStream,
    sync::{broadcast, oneshot},
    task,
};
use tonic::transport::{Channel, Endpoint, Server, Uri};
use tower::service_fn;
use tracing_core::LevelFilter;
use tracing_subscriber::{prelude::*, EnvFilter};

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

#[track_caller]
pub fn assert_tasks<Fut>(expected_tasks: Vec<ExpectedTask>, future: Fut)
where
    Fut: Future + Send + 'static,
    Fut::Output: Send + 'static,
{
    let (client_stream, server_stream) = tokio::io::duplex(1024);

    let (console_layer, server) = console_subscriber::ConsoleLayer::builder().build();

    let registry = tracing_subscriber::registry().with(console_layer);

    let (finish_tx, mut finish_rx) = broadcast::channel(1);

    let (client_connected_tx, client_connected_rx) = oneshot::channel();

    let join_handle = thread::Builder::new()
        .name("console_subscriber".into())
        .spawn(move || {
            let file = std::fs::File::create("console_subscriber.log")
                .expect("Couldn't create temporary log file");
            let sub = tracing_subscriber::fmt()
                .with_writer(file)
                .with_env_filter(
                    EnvFilter::builder()
                        .with_default_directive(LevelFilter::DEBUG.into())
                        .parse_lossy("wake=trace,console_subscriber=trace,info"),
                )
                .finish();
            let _subscriber_guard = tracing::subscriber::set_default(sub);
            // tracing::subscriber::set_default(tracing_core::subscriber::NoSubscriber::default());
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_io()
                .enable_time()
                .build()
                .expect("console-test error: failed to initialize console subscriber runtime");

            runtime.block_on(async move {
                task::Builder::new()
                    .name("console-server")
                    .spawn(console_server(server, server_stream, finish_tx.subscribe()))
                    .expect("console-test error: could not spawn 'console-server' task");

                let console_client = task::Builder::new()
                    .name("console-client")
                    .spawn(async move {
                        tracing::debug!("#### console-client: before sleep");
                        // client_connected_rx
                        //     .await
                        //     .expect("console-test error: Failure awaiting start signal");
                        // tokio::time::sleep(std::time::Duration::from_millis(200)).await;
                        tracing::debug!("#### console-client: after sleep");

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
                        tracing::debug!("### console-client: before send client connected");
                        client_connected_tx
                            .send(())
                            .expect("console-client error: couldn't send client connected signal");
                        tracing::debug!("### console-client: after send client connected");

                        tracing::debug!("#### console-client: before record actual tasks");
                        let actual_tasks = record_actual_tasks(channel, 4).await;
                        tracing::debug!("#### console-client: after record actual tasks");

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

                        finish_tx
                            .send(())
                            .expect("console-test error: failed to send completion message");

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
            tracing::debug!("**** Before await client connected...");

            client_connected_rx
                .await
                .expect("console-test error: couldn't receive client connected signal");

            tracing::debug!("**** After await client connected...");

            // Run the future that we are testing.
            _ = tokio::task::Builder::new()
                .name(MAIN_TASK_NAME)
                .spawn(future)
                .expect("console-test error: couldn't spawn test task")
                .await;

            // client_connected_tx
            //     .send(())
            //     .expect("console-test error: Could not send start signal");
            tracing::debug!("**** After spawn task away - Before await finish...");
            match finish_rx.recv().await {
                Ok(_) => tracing::debug!("finish message received."),
                Err(err) => tracing::debug!("finish message could not be received: {err}"),
            }
            tracing::debug!("**** After await finish");
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
    mut completion_rx: broadcast::Receiver<()>,
) {
    let (service, aggregate) = server.into_parts();
    Server::builder()
        .add_service(service)
        .serve_with_incoming(futures::stream::iter(vec![Ok::<_, std::io::Error>(
            server_stream,
        )]))
        .await
        .expect("console subscriber failed.");
    tracing::debug!("Waiting for finish signal");
    match completion_rx.recv().await {
        Ok(_) => tracing::debug!("Getting ready to drop the aggregate handle."),
        Err(err) => tracing::debug!("Error waiting for finish signal: {err:?}"),
    }
    drop(aggregate);
}

async fn record_actual_tasks(channel: Channel, update_limit: usize) -> Vec<ActualTask> {
    let mut client = InstrumentClient::new(channel);

    let mut stream = loop {
        let request = tonic::Request::new(InstrumentRequest {});
        match client.watch_updates(request).await {
            Ok(stream) => break stream.into_inner(),
            Err(err) => panic!("Client cannot connect to watch updates: {err}"),
        }
    };

    let mut tasks = HashMap::new();

    let mut i: usize = 0;
    while let Some(update) = stream.next().await {
        match update {
            Ok(update) => {
                tracing::debug!("----==== UPDATE {i} ====----");
                if let Some(register_metadata) = &update.new_metadata {
                    for new_metadata in &register_metadata.metadata {
                        if let Some(metadata) = &new_metadata.metadata {
                            //tracing::debug!("New metadata: name: {:?}", metadata.name);
                            if metadata.name == "runtime.spawn" {
                                // tracing::debug!("New metadata: {:?}", metadata);
                            }
                        }
                    }
                }
                if let Some(task_update) = &update.task_update {
                    for new_task in &task_update.new_tasks {
                        if let Some(id) = &new_task.id {
                            let mut actual_task = ActualTask::new(id.id);
                            tracing::debug!("NEW TASK -> id = {id:?}");
                            for field in &new_task.fields {
                                if let Some(console_api::field::Name::StrName(field_name)) =
                                    &field.name
                                {
                                    tracing::debug!("  -> {field:?}");
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
                        } else {
                            tracing::debug!("New task without ID");
                        }
                    }

                    for (id, stats) in &task_update.stats_update {
                        if let Some(mut task) = tasks.get_mut(id) {
                            task.wakes = stats.wakes;
                            task.self_wakes = stats.self_wakes;
                        }
                        tracing::debug!("{id} --> {stats:?}");
                    }
                }

                if let Some(task_update) = update.task_update {
                    tracing::debug!(
                        "UPDATE: new task count: {}, update count: {}, dropped count: {}",
                        task_update.new_tasks.len(),
                        task_update.stats_update.len(),
                        task_update.dropped_events
                    );
                }
                i += 1;
            }
            Err(e) => {
                panic!("update stream error: {}", e);
            }
        }
        if i > update_limit {
            break;
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
