use std::time::Duration;

static HELP: &str = r#"
Example console-instrumented app featuring tokio and rayon

USAGE:
    rayon-app [OPTIONS]

OPTIONS:
    -h, help    prints this message
"#;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let mut env = std::env::var("RUST_LOG")
        .map(|mut s| {
            s.push(',');
            s
        })
        .unwrap_or_default();
    env.push_str("rayon_core=trace");
    std::env::set_var("RUST_LOG", env);
    console_subscriber::init();
    // spawn optional extras from CLI args
    // skip first which is command name
    #[allow(clippy::never_loop)]
    for opt in std::env::args().skip(1) {
        match &*opt {
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

    tokio::spawn(spawn_tasks(1, 10)).await?;

    Ok(())
}

#[tracing::instrument]
async fn spawn_tasks(min: u64, max: u64) {
    loop {
        for i in min..max {
            tokio::task::spawn_blocking(move || rayon_join_recursively(i * 10))
                .await
                .unwrap();
            tokio::time::sleep(Duration::from_secs(max) - Duration::from_secs(i)).await;
        }
    }
}

#[tracing::instrument]
fn rayon_sum_of_squares(max: u64) {
    use rayon::prelude::*;
    let sum: u64 = (0..max).into_par_iter().map(|i| i * i).sum();
    tracing::trace!(max, sum);
}

#[tracing::instrument]
fn rayon_join_recursively(max: u64) {
    fn join_recursively(n: u64) {
        if n == 0 {
            return;
        }
        rayon::join(|| join_recursively(n - 1), || join_recursively(n - 1));
    }

    rayon::spawn(move || join_recursively(max * 100))
}
