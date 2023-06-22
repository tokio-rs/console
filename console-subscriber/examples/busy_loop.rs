use std::sync::atomic::Ordering;
use std::sync::{atomic::AtomicBool, Arc};
use std::time::Duration;
use tokio::task;
use tokio::time::sleep;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    console_subscriber::init();
    let stop = Arc::new(AtomicBool::new(false));

    let t = task::Builder::default()
        .name("busy-loop")
        .spawn({
            let stop = Arc::clone(&stop);
            async move {
                loop {
                    if stop.load(Ordering::Acquire) {
                        break;
                    }
                }
            }
        })
        .unwrap();

    sleep(Duration::from_secs(300)).await;
    stop.store(true, Ordering::Release);
    t.await?;

    Ok(())
}
