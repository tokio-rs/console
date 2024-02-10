//! Example of using the console subscriber with tonic-web.
//! This example requires the `grpc-web` feature to be enabled.
//! Run with:
//! ```sh
//! cargo run --example grpc_web --features grpc-web
//! ```
use std::{thread, time::Duration};

use console_subscriber::{ConsoleLayer, ServerParts};
use http::header::HeaderName;
use tonic_web::GrpcWebLayer;
use tower_http::cors::{AllowOrigin, CorsLayer};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

const DEFAULT_MAX_AGE: Duration = Duration::from_secs(24 * 60 * 60);
const DEFAULT_EXPOSED_HEADERS: [&str; 3] =
    ["grpc-status", "grpc-message", "grpc-status-details-bin"];
const DEFAULT_ALLOW_HEADERS: [&str; 5] = [
    "x-grpc-web",
    "content-type",
    "x-user-agent",
    "grpc-timeout",
    "user-agent",
];

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let (console_layer, server) = ConsoleLayer::builder().with_default_env().build();
    thread::Builder::new()
        .name("subscriber".into())
        .spawn(move || {
            // Do not trace anything in this thread.
            let _subscriber_guard =
                tracing::subscriber::set_default(tracing_core::subscriber::NoSubscriber::default());
            // Custom CORS configuration.
            let cors = CorsLayer::new()
                .allow_origin(AllowOrigin::mirror_request())
                .allow_credentials(true)
                .max_age(DEFAULT_MAX_AGE)
                .expose_headers(
                    DEFAULT_EXPOSED_HEADERS
                        .iter()
                        .cloned()
                        .map(HeaderName::from_static)
                        .collect::<Vec<HeaderName>>(),
                )
                .allow_headers(
                    DEFAULT_ALLOW_HEADERS
                        .iter()
                        .cloned()
                        .map(HeaderName::from_static)
                        .collect::<Vec<HeaderName>>(),
                );
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("console subscriber runtime initialization failed");
            runtime.block_on(async move {
                let ServerParts {
                    instrument_server,
                    aggregator,
                    ..
                } = server.into_parts();
                tokio::spawn(aggregator.run());
                let router = tonic::transport::Server::builder()
                    // Accept gRPC-Web requests and enable CORS.
                    .accept_http1(true)
                    .layer(cors)
                    .layer(GrpcWebLayer::new())
                    .add_service(instrument_server);
                let serve = router.serve(std::net::SocketAddr::new(
                    std::net::IpAddr::V4(std::net::Ipv4Addr::new(127, 0, 0, 1)),
                    9999,
                ));
                serve.await.expect("console subscriber server failed");
            });
        })
        .expect("console subscriber could not spawn thread");
    tracing_subscriber::registry().with(console_layer).init();

    let task1 = tokio::task::Builder::new()
        .name("task1")
        .spawn(spawn_tasks(1, 10))
        .unwrap();
    let task2 = tokio::task::Builder::new()
        .name("task2")
        .spawn(spawn_tasks(10, 30))
        .unwrap();

    let result = tokio::try_join! {
        task1,
        task2,
    };
    result?;

    Ok(())
}

#[tracing::instrument]
async fn spawn_tasks(min: u64, max: u64) {
    loop {
        for i in min..max {
            tracing::trace!(i, "spawning wait task");
            tokio::task::Builder::new()
                .name("wait")
                .spawn(wait(i))
                .unwrap();

            let sleep = Duration::from_secs(max) - Duration::from_secs(i);
            tracing::trace!(?sleep, "sleeping...");
            tokio::time::sleep(sleep).await;
        }
    }
}

#[tracing::instrument]
async fn wait(seconds: u64) {
    tracing::debug!("waiting...");
    tokio::time::sleep(Duration::from_secs(seconds)).await;
    tracing::trace!("done!");
}
