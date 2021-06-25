use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    console_subscriber::init();

    let task1 = tokio::spawn(spawn_tasks(1, 10));
    let task2 = tokio::spawn(spawn_tasks(10, 30));

    // spawn optional extras from CLI args
    // skip first which is command name
    for opt in std::env::args().skip(1) {
        match &*opt {
            "blocks" => {
                tokio::spawn(double_sleepy(1, 10));
            }
            "coma" => {
                tokio::spawn(std::future::pending::<()>());
            }
            "burn" => {
                tokio::spawn(burn(1, 10));
            }
            wat => return Err(format!("unknown option: {:?}", wat).into()),
        }
    }

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
