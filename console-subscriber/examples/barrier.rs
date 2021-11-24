use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Barrier;
use tokio::task;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    console_subscriber::init();
    task::Builder::default()
        .name("main-task")
        .spawn(async move {
            let mut handles = Vec::with_capacity(30);
            let barrier = Arc::new(Barrier::new(30));
            for i in 0..30 {
                let c = barrier.clone();
                let task_name = format!("task-{}", i);
                handles.push(task::Builder::default().name(&task_name).spawn(async move {
                    tokio::time::sleep(Duration::from_secs(i)).await;
                    let wait_result = c.wait().await;
                    wait_result
                }));
            }

            // Will not resolve until all "after wait" messages have been printed
            let mut num_leaders = 0;
            for handle in handles {
                let wait_result = handle.await.unwrap();
                if wait_result.is_leader() {
                    num_leaders += 1;
                }
            }

            tokio::time::sleep(Duration::from_secs(10)).await;
            // Exactly one barrier will resolve as the "leader"
            assert_eq!(num_leaders, 1);
        })
        .await?;

    Ok(())
}
