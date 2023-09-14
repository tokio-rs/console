use std::time::Duration;

static HELP: &str = r#"
Example console-instrumented app

USAGE:
    app [OPTIONS]

OPTIONS:
    -h, help    prints this message
    blocks      Includes a (misbehaving) blocking task
    burn        Includes a (misbehaving) task that spins CPU with self-wakes
    coma        Includes a (misbehaving) task that forgets to register a waker
    noyield     Includes a (misbehaving) task that spawns tasks that never yield
"#;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    console_subscriber::init();
    // spawn optional extras from CLI args
    // skip first which is command name
    for opt in std::env::args().skip(1) {
        match &*opt {
            "blocks" => {
                tokio::task::Builder::new()
                    .name("blocks")
                    .spawn(double_sleepy(1, 10))
                    .unwrap();
            }
            "coma" => {
                tokio::task::Builder::new()
                    .name("coma")
                    .spawn(std::future::pending::<()>())
                    .unwrap();
            }
            "burn" => {
                tokio::task::Builder::new()
                    .name("burn")
                    .spawn(burn(1, 10))
                    .unwrap();
            }
            "noyield" => {
                tokio::task::Builder::new()
                    .name("noyield")
                    .spawn(no_yield(20))
                    .unwrap();
            }
            "help" | "-h" => {
                eprintln!("{}", HELP);
                return Ok(());
            }
            wat => {
                return Err(
                    format!("unknown option: {:?}, run with '-h' to see options", wat).into(),
                )
            }
        }
    }

    let task1 = tokio::task::Builder::new()
        .name("task1")
        .spawn(spawn_tasks(1, 10))
        .unwrap();
    let task2 = tokio::task::Builder::new()
        .name("task2")
        .spawn(spawn_tasks(10, 30))
        .unwrap();

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
            tracing::trace!(i, "spawning wait task");
            tokio::task::Builder::new()
                .name("wait")
                .spawn(wait(i))
                .unwrap();

            let sleep = Duration::from_secs(max) - Duration::from_secs(i);
            tracing::trace!(?sleep, "sleeping...");
            tokio::time::sleep(sleep).await;
        }
    }
}

#[tracing::instrument]
async fn wait(seconds: u64) {
    tracing::debug!("waiting...");
    tokio::time::sleep(Duration::from_secs(seconds)).await;
    tracing::trace!("done!");
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

#[tracing::instrument]
async fn no_yield(seconds: u64) {
    loop {
        let handle = tokio::task::Builder::new()
            .name("greedy")
            .spawn(async move {
                std::thread::sleep(Duration::from_secs(seconds));
            })
            .expect("Couldn't spawn greedy task");

        _ = handle.await;
    }
}
