//! Long scheduled time
//!
//! This example shows an application with a task that has an excessive
//! time between being woken and being polled.
//!
//! It consists of a channel where a sender task sends a message
//! through the channel and then immediately does a lot of work
//! (simulated in this case by a call to `std::thread::sleep`).
//!
//! As soon as the sender task calls `send()` the receiver task gets
//! woken, but because there's only a single worker thread, it doesn't
//! get polled until after the sender task has finished "working" and
//! yields (via `tokio::time::sleep`).
use std::time::Duration;

use console_subscriber::ConsoleLayer;
use tokio::{sync::mpsc, task};
use tracing::info;

#[tokio::main(flavor = "multi_thread", worker_threads = 1)]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    ConsoleLayer::builder()
        .with_default_env()
        .publish_interval(Duration::from_millis(100))
        .init();

    let (tx, rx) = mpsc::channel::<u32>(1);
    let count = 10000;

    let jh_rx = task::Builder::new()
        .name("rx")
        .spawn(receiver(rx, count))
        .unwrap();
    let jh_tx = task::Builder::new()
        .name("tx")
        .spawn(sender(tx, count))
        .unwrap();

    let res_tx = jh_tx.await;
    let res_rx = jh_rx.await;
    info!(
        "main: Joined sender: {:?} and receiver: {:?}",
        res_tx, res_rx,
    );

    tokio::time::sleep(Duration::from_millis(200)).await;

    Ok(())
}

async fn sender(tx: mpsc::Sender<u32>, count: u32) {
    info!("tx: started");

    for idx in 0..count {
        let msg: u32 = idx;
        let res = tx.send(msg).await;
        info!("tx: sent msg '{}' result: {:?}", msg, res);

        std::thread::sleep(Duration::from_millis(5000));
        info!("tx: work done");

        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

async fn receiver(mut rx: mpsc::Receiver<u32>, count: u32) {
    info!("rx: started");

    for _ in 0..count {
        let msg = rx.recv().await;
        info!("rx: Received message: '{:?}'", msg);
    }
}
