use std::{thread, time::Duration};

use console_api::instrument::{instrument_client::InstrumentClient, InstrumentRequest};
use futures::stream::StreamExt;
use tokio::{sync::oneshot, task, time::sleep};
use tracing_subscriber::prelude::*;

#[test]
fn self_wake() {
    let (console_layer, server) = console_subscriber::ConsoleLayer::builder().build();

    let registry = tracing_subscriber::registry().with(console_layer);

    let (finish_tx, finish_rx) = oneshot::channel::<()>();

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

            runtime
                .block_on(async move {
                    task::Builder::new()
                        .name("console_server")
                        .spawn(async move {
                            server
                                .serve()
                                .await
                                .expect("console subscriber server failed")
                        })
                        .unwrap();

                    let expect = task::Builder::new().name("expect").spawn(async {
                        tokio::time::sleep(std::time::Duration::from_millis(200)).await;

                        let target = "http://127.0.0.1:6669".to_owned();
                        let mut client = InstrumentClient::connect(target).await.unwrap();


                        let mut fail_count = 0;
                        let mut stream = loop {
                            let request = tonic::Request::new(InstrumentRequest {});
                            match client.watch_updates(request).await {
                                Ok(stream) => break stream.into_inner(),
                                Err(err) => {
                                    if fail_count < 5 {
                                        fail_count += 1;
                                        println!(
                                        "Could not connect ({fail_count}), will try again: {err}"
                                    );
                                        sleep(Duration::from_millis(fail_count * 100)).await;
                                    } else {
                                        panic!("Client cannot connect to watch updates: {err}");
                                    }
                                }
                            }
                        };

                        let mut i: usize = 0;
                        while let Some(update) = stream.next().await {
                            match update {
                                Ok(update) => {
                                    println!("UPDATE {}: {:#?}\n", i, update.task_update);
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
                        match finish_tx.send(()) {
                            Ok(_) => println!("Send finish message!"),
                            Err(err) => println!("Could not send finish message: {err:?}"),
                        }
                    }).unwrap();

                    match expect.await {
                        Ok(_) => println!("Successfully awaited expect task!"),
                        Err(err) => println!("Error awaiting expect task: {err:?}"),
                    }
                });
        })
        .expect("console subscriber could not spawn thread");

    tracing::subscriber::with_default(registry, || {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();

        tokio::task::Builder::new()
            .name("mog")
            .spawn_on(async { task::yield_now().await }, runtime.handle())
            .unwrap();
        runtime.block_on(async {
            println!("Before await finish...");
            match finish_rx.await {
                Ok(_) => println!("finish message received."),
                Err(err) => println!("finish message could not be received: {err}"),
            }
        });
    });

    match join_handle.join() {
        Ok(_) => println!("Successfully joined console subscriber thread"),
        Err(err) => println!("Error joining console subscriber thread: {err:?}"),
    }
}
