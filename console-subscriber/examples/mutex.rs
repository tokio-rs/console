use std::sync::Arc;
use std::time::Duration;
use tokio::{sync::Mutex, task};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    console_subscriber::init();
    task::Builder::default()
        .name("main-task")
        .spawn(async move {
            let count = Arc::new(Mutex::new(0));
            for i in 0..5 {
                let my_count = Arc::clone(&count);
                let task_name = format!("increment-{}", i);
                tokio::task::Builder::default()
                    .name(&task_name)
                    .spawn(async move {
                        for _ in 0..10 {
                            let mut lock = my_count.lock().await;
                            *lock += 1;
                            tokio::time::sleep(Duration::from_secs(1)).await;
                        }
                    });
            }

            loop {
                if *count.lock().await >= 50 {
                    break;
                }
            }
        })
        .await?;

    Ok(())
}
