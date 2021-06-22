use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    console_subscriber::init();

    let task1 = tokio::spawn(spawn_tasks(1, 10));
    let task2 = tokio::spawn(spawn_tasks(10, 100));
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
            tokio::spawn(wait(i));
            tokio::time::sleep(Duration::from_secs(max) - Duration::from_secs(i)).await;
        }
    }
}

#[tracing::instrument]
async fn wait(seconds: u64) {
    tokio::time::sleep(Duration::from_secs(seconds)).await;
}
