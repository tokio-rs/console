use std::sync::Arc;
use std::time::Duration;
use tokio::task;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    console_subscriber::init();
    task::Builder::default()
        .name("main-task")
        .spawn(async move {
            let sem = Arc::new(tokio::sync::Semaphore::new(0));
            let mut tasks = Vec::default();
            for i in 0..5 {
                let acquire_sem = Arc::clone(&sem);
                let add_sem = Arc::clone(&sem);
                let acquire_task_name = format!("acquire-{}", i);
                let add_task_name = format!("add-{}", i);
                tasks.push(
                    tokio::task::Builder::default()
                        .name(&acquire_task_name)
                        .spawn(async move {
                            let _permit = acquire_sem.acquire_many(i).await.unwrap();
                            tokio::time::sleep(Duration::from_secs(i as u64 * 2)).await;
                        })
                        .unwrap(),
                );
                tasks.push(
                    tokio::task::Builder::default()
                        .name(&add_task_name)
                        .spawn(async move {
                            tokio::time::sleep(Duration::from_secs(i as u64 * 5)).await;
                            add_sem.add_permits(i as usize);
                        })
                        .unwrap(),
                );
            }

            let all_tasks = futures::future::try_join_all(tasks);
            all_tasks.await.unwrap();
        })
        .unwrap()
        .await?;

    Ok(())
}
