use std::sync::Arc;
use std::time::Duration;
use tokio::{sync::RwLock, task};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    console_subscriber::init();
    task::Builder::default()
        .name("main-task")
        .spawn(async move {
            let count = Arc::new(RwLock::new(0));
            for i in 0..5 {
                let my_count = Arc::clone(&count);
                let task_name = format!("increment-{}", i);
                tokio::task::Builder::default()
                    .name(&task_name)
                    .spawn(async move {
                        for _ in 0..10 {
                            let mut lock = my_count.write().await;
                            *lock += 1;
                            tokio::time::sleep(Duration::from_secs(1)).await;
                        }
                    });
            }

            loop {
                let c = count.read().await;
                tokio::time::sleep(Duration::from_secs(1)).await;
                if *c >= 50 {
                    break;
                }
            }
        })
        .await?;

    Ok(())
}
