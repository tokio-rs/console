use std::time::Duration;

use console_subscriber::ConsoleLayer;
use tokio::task::{self, yield_now};
use tracing::info;

#[tokio::main(flavor = "multi_thread", worker_threads = 2)]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    ConsoleLayer::builder()
        .with_default_env()
        .publish_interval(Duration::from_millis(100))
        .init();

    let long_sleeps = task::Builder::new()
        .name("long-sleeps")
        .spawn(long_sleeps(5000))
        .unwrap();

    let sleep_forever = task::Builder::new()
        .name("sleep-forever")
        .spawn(sleep_forever(5000))
        .unwrap();

    match (long_sleeps.await, sleep_forever.await) {
        (Ok(_), Ok(_)) => info!("Success"),
        (_, _) => info!("Error awaiting tasks."),
    }

    tokio::time::sleep(Duration::from_millis(200)).await;

    Ok(())
}

async fn long_sleeps(inc: u64) {
    let millis = inc;
    loop {
        std::thread::sleep(Duration::from_millis(millis));

        yield_now().await;
    }
}

async fn sleep_forever(inc: u64) {
    let millis = inc;
    loop {
        std::thread::sleep(Duration::from_millis(millis));
    }
}
