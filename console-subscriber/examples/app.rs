use std::time::Duration;
use tracing_subscriber::prelude::*;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    assert!(
        cfg!(tokio_unstable),
        "task tracing requires Tokio to be built with RUSTFLAGS=\"--cfg tokio_unstable\"!"
    );

    let (layer, server) = console_subscriber::TasksLayer::new();
    let filter =
        tracing_subscriber::EnvFilter::from_default_env().add_directive("tokio=trace".parse()?);
    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer())
        .with(filter)
        .with(layer)
        .init();

    let serve = tokio::spawn(async move { server.serve().await.expect("server failed") });
    let task1 = tokio::spawn(spawn_tasks(1, 10));
    let task2 = tokio::spawn(spawn_tasks(10, 100));
    let result = tokio::try_join! {
        task1,
        task2,
        serve
    };
    result?;

    Ok(())
}

#[tracing::instrument]
async fn spawn_tasks(min: u64, max: u64) {
    loop {
        for i in min..max {
            tokio::spawn(wait(i));
            tokio::time::sleep(Duration::from_secs(max) - Duration::from_secs(i)).await;
        }
    }
}

#[tracing::instrument]
async fn wait(seconds: u64) {
    tokio::time::sleep(Duration::from_secs(seconds)).await;
}
