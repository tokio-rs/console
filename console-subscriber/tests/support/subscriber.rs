use std::{collections::HashMap, future::Future, thread, time::Duration, fmt};

use console_api::{
    field::Value,
    instrument::{instrument_client::InstrumentClient, InstrumentRequest},
};
use futures::stream::StreamExt;
use tokio::{sync::broadcast, task};
use tonic::transport::{Endpoint, Server, Uri};
use tower::service_fn;
use tracing_subscriber::prelude::*;

use super::task::{ActualTask, ExpectedTask, TaskValidationFailure};

#[derive(Debug)]
enum TestFailure {
    NoTasksMatched,
    TasksFailedValidation { failures: Vec<TaskValidationFailure> },
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
    // Everything else is here.
    let (client_stream, server_stream) = tokio::io::duplex(1024);

    let (console_layer, server) = console_subscriber::ConsoleLayer::builder().build();

    let registry = tracing_subscriber::registry().with(console_layer);

    let (finish_tx, mut finish_rx) = broadcast::channel(1);

    let join_handle = thread::Builder::new()
        .name("console_subscriber".into())
        .spawn(move || {
            let _subscriber_guard =
                tracing::subscriber::set_default(tracing_core::subscriber::NoSubscriber::default());
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_io()
                .enable_time()
                .build()
                .expect("console subscriber runtime initialization failed");

            let mut console_server_finish_rx = finish_tx.subscribe();
            runtime
                .block_on(async move {
                    task::Builder::new()
                        .name("console_server")
                        .spawn(async move {
                            let (service, aggregate) = server.into_parts();
                            Server::builder()
                                .add_service(service)
                                .serve_with_incoming(futures::stream::iter(vec![Ok::<_, std::io::Error>(server_stream)]))
                                .await
                                .expect("console subscriber failed.");
                            println!("Waiting for finish signal");
                            match console_server_finish_rx.recv().await {
                                Ok(_) => println!("Getting ready to drop the aggregate handle."),
                                Err(err) => println!("Error waiting for finish signal: {err:?}"),
                            }
                            drop(aggregate);
                        })
                        .unwrap();

                    let expect = task::Builder::new().name("expect").spawn(async move {
                        tokio::time::sleep(Duration::from_millis(200)).await;

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
                        .expect("Could not create client");

                        let mut client = InstrumentClient::new(channel);

                        let mut stream = loop {
                            let request = tonic::Request::new(InstrumentRequest {});
                            match client.watch_updates(request).await {
                                Ok(stream) => break stream.into_inner(),
                                Err(err) => panic!("Client cannot connect to watch updates: {err}"),
                            }
                        };

                        let mut tasks = HashMap::new();

                        // let expected_task = ExpectedTask::default().match_name("mog".into()).expect_wakes(1).expect_self_wakes(0);
                        // let expected_tasks = vec![expected_task];

                        let mut i: usize = 0;
                        while let Some(update) = stream.next().await {
                            match update {
                                Ok(update) => {
                                    println!("----==== UPDATE {i} ====----");
                                    if let Some(register_metadata) = &update.new_metadata {
                                        for new_metadata in &register_metadata.metadata {
                                            if let Some(metadata) = &new_metadata.metadata {
                                                println!("New metadata: name: {:?}", metadata.name);
                                                if metadata.name == "runtime.spawn" {
                                                    println!("New metadata: {:?}", metadata);
                                                }
                                            }
                                        }
                                        // println!("metadata: {metadata:#?}");
                                    }
                                    if let Some(task_update) = &update.task_update {
                                        for new_task in &task_update.new_tasks {
                                            // println!("New task: {new_task:#?}");
                                            println!("New task!");

                                            if let Some(id) = &new_task.id {
                                                let mut actual_task = ActualTask::new(id.id);
                                                println!("  -> id = {id:?}");
                                                for field in &new_task.fields {
                                                    if let Some(console_api::field::Name::StrName(field_name)) = &field.name {
                                                        println!("  -> {field:?}");
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
                                            println!("{id} --> {stats:?}");
                                        }
                                    }

                                    if let Some(task_update) = update.task_update {
                                        println!(
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
                            if i > 2 {
                                break;
                            }
                        }

                        let mut validation_results = Vec::new();
                        // let mut test_passes = None;
                        for expected in &expected_tasks {
                            for (_, actual) in &tasks {
                                if expected.matches_actual_task(actual) {

                                    validation_results.push(expected.validate_actual_task(actual));
                                    
                                    // We only match a single task.
                                    // FIXME(hds): We should probably create an error or a warning if multiple tasks match.
                                    continue; 
                                }
                            }
                        }

                        let result = if validation_results.is_empty() {
                            Err(TestFailure::NoTasksMatched)
                        }
                        else {
                            let failures: Vec<_> = validation_results.into_iter().filter_map(|r| match r {
                                Ok(_) => None,
                                Err(validation_error) => Some(validation_error),
                            }).collect();

                            if failures.is_empty() {
                                Ok(())
                            } else {
                                Err(TestFailure::TasksFailedValidation { failures: failures })
                            }
                        };
                        // match test_passes {
                        //     Some(true) => println!("Test passes!!!"),
                        //     Some(false) => println!("Test fails!!!"),
                        //     None => println!("Nothing was tested..."),
                        // }

                        // println!("{tasks:#?}");
                        match finish_tx.send(()) {
                            Ok(_) => println!("Send finish message!"),
                            Err(err) => println!("Could not send finish message: {err:?}"),
                        }

                        result
                    }).unwrap();

                    match expect.await {
                        Ok(test_result) => test_result,
                        Err(err) => panic!("Error awaiting expect task: {err:?}"),
                    }
                })
        })
        .expect("console subscriber could not spawn thread");

    tracing::subscriber::with_default(registry, || {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();

        tokio::task::Builder::new()
            .name("mog")
            .spawn_on(future, runtime.handle())
            .unwrap();
        runtime.block_on(async {
            println!("Before await finish...");
            match finish_rx.recv().await {
                Ok(_) => println!("finish message received."),
                Err(err) => println!("finish message could not be received: {err}"),
            }
        });
    });

    match join_handle.join() {
        Ok(Ok(_)) => println!("Test was successful!"),
        Ok(Err(test_failure)) => panic!("Test failed: {test_failure}"),
        Err(err) => panic!("Error joining console subscriber thread: {err:?}"),
    }
}
