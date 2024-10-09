use std::{future::Future, task::Poll, time::Duration};

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
    blocking    Includes a blocking task that  (not misbehaving)
    large       Includes tasks that are driven by futures that are larger than recommended
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
            "blocking" => {
                tokio::task::Builder::new()
                    .name("spawns_blocking")
                    .spawn(spawn_blocking(5))
                    .unwrap();
            }
            "large" => {
                tokio::task::Builder::new()
                    .name("pretty-big")
                    // Below debug mode auto-boxing limit
                    .spawn(large_future::<1024>())
                    .unwrap();
                tokio::task::Builder::new()
                    .name("huge")
                    // Larger than the release mode auto-boxing limit
                    .spawn(large_future::<20_000>())
                    .unwrap();
                large_blocking::<20_000>();
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
                self_wake().await;
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

#[tracing::instrument]
async fn spawn_blocking(seconds: u64) {
    loop {
        _ = tokio::task::spawn_blocking(move || {
            std::thread::sleep(Duration::from_secs(seconds));
        })
        .await;
    }
}

#[tracing::instrument]
async fn large_future<const N: usize>() {
    let mut numbers = [0_u8; N];

    loop {
        for idx in 0..N {
            numbers[idx] = (idx % 256) as u8;
            tokio::time::sleep(Duration::from_millis(100)).await;
            (0..=idx).for_each(|jdx| {
                assert_eq!(numbers[jdx], (jdx % 256) as u8);
            });
        }
    }
}

fn large_blocking<const N: usize>() {
    let numbers = [0_u8; N];

    tokio::task::Builder::new()
        .name("huge-blocking")
        .spawn_blocking(move || {
            let mut numbers = numbers;

            loop {
                for idx in 0..N {
                    numbers[idx] = (idx % 256) as u8;
                    std::thread::sleep(Duration::from_millis(100));
                    (0..=idx).for_each(|jdx| {
                        assert_eq!(numbers[jdx], (jdx % 256) as u8);
                    });
                }
            }
        })
        .unwrap();
}

fn self_wake() -> impl Future<Output = ()> {
    struct SelfWake {
        yielded: bool,
    }

    impl Future for SelfWake {
        type Output = ();

        fn poll(
            mut self: std::pin::Pin<&mut Self>,
            cx: &mut std::task::Context<'_>,
        ) -> Poll<Self::Output> {
            if self.yielded {
                return Poll::Ready(());
            }

            self.yielded = true;
            cx.waker().wake_by_ref();

            Poll::Pending
        }
    }

    SelfWake { yielded: false }
}
