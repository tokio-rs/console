//! Example of using the console subscriber with tonic-web.
//! This example requires the `grpc-web` feature to be enabled.
//! Run with:
//! ```sh
//! cargo run --example grpc_web --features grpc-web
//! ```
use std::time::Duration;

use console_subscriber::ConsoleLayer;
use http::header::HeaderName;
use tower_http::cors::{AllowOrigin, CorsLayer};

static HELP: &str = r#"
Example console-instrumented app

USAGE:
    app [OPTIONS]

OPTIONS:
    -h, help    prints this message
    blocks      Includes a (misbehaving) blocking task
    burn        Includes a (misbehaving) task that spins CPU with self-wakes
    coma        Includes a (misbehaving) task that forgets to register a waker
    noyield     Includes a (misbehaving) task that spawns tasks that never yield
"#;

const DEFAULT_MAX_AGE: Duration = Duration::from_secs(24 * 60 * 60);
const DEFAULT_EXPOSED_HEADERS: [&str; 3] =
    ["grpc-status", "grpc-message", "grpc-status-details-bin"];
const DEFAULT_ALLOW_HEADERS: [&str; 4] =
    ["x-grpc-web", "content-type", "x-user-agent", "grpc-timeout"];

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
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
    ConsoleLayer::builder()
        .with_default_env()
        .with_cors(cors)
        .init();
    // spawn optional extras from CLI args
    // skip first which is command name
    for opt in std::env::args().skip(1) {
        match &*opt {
            "blocks" => {
                tokio::task::Builder::new()
                    .name("blocks")
                    .spawn(double_sleepy(1, 10))
                    .unwrap();
            }
            "coma" => {
                tokio::task::Builder::new()
                    .name("coma")
                    .spawn(std::future::pending::<()>())
                    .unwrap();
            }
            "burn" => {
                tokio::task::Builder::new()
                    .name("burn")
                    .spawn(burn(1, 10))
                    .unwrap();
            }
            "noyield" => {
                tokio::task::Builder::new()
                    .name("noyield")
                    .spawn(no_yield(20))
                    .unwrap();
            }
            "blocking" => {
                tokio::task::Builder::new()
                    .name("spawns_blocking")
                    .spawn(spawn_blocking(5))
                    .unwrap();
            }
            "help" | "-h" => {
                eprintln!("{}", HELP);
                return Ok(());
            }
            wat => {
                return Err(
                    format!("unknown option: {:?}, run with '-h' to see options", wat).into(),
                )
            }
        }
    }

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

#[tracing::instrument]
async fn double_sleepy(min: u64, max: u64) {
    loop {
        for i in min..max {
            // woops!
            std::thread::sleep(Duration::from_secs(i));
            tokio::time::sleep(Duration::from_secs(max - i)).await;
        }
    }
}

#[tracing::instrument]
async fn burn(min: u64, max: u64) {
    loop {
        for i in min..max {
            for _ in 0..i {
                tokio::task::yield_now().await;
            }
            tokio::time::sleep(Duration::from_secs(i - min)).await;
        }
    }
}

#[tracing::instrument]
async fn no_yield(seconds: u64) {
    loop {
        let handle = tokio::task::Builder::new()
            .name("greedy")
            .spawn(async move {
                std::thread::sleep(Duration::from_secs(seconds));
            })
            .expect("Couldn't spawn greedy task");

        _ = handle.await;
    }
}

#[tracing::instrument]
async fn spawn_blocking(seconds: u64) {
    loop {
        let seconds = seconds;
        _ = tokio::task::spawn_blocking(move || {
            std::thread::sleep(Duration::from_secs(seconds));
        })
        .await;
    }
}
