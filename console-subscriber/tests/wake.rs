use std::{thread, time::Duration};

use console_api::instrument::{instrument_client::InstrumentClient, InstrumentRequest};
use futures::stream::StreamExt;
use tokio::{sync::broadcast, task};
use tonic::transport::{Endpoint, Server, Uri};
use tower::service_fn;
use tracing_subscriber::prelude::*;

#[test]
fn self_wake() {
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

                        let mut i: usize = 0;
                        while let Some(update) = stream.next().await {
                            match update {
                                Ok(update) => {
                                    println!("UPDATE {}: {:#?}", i, update.task_update);
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
            match finish_rx.recv().await {
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
