//! Local tasks
//!
//! This example shows the instrumentation on local tasks. Tasks spawned onto a
//! `LocalSet` with `spawn_local` have the kind `local` in `tokio-console`.
//!
//! Additionally, because the `console-subscriber` is initialized before the
//! tokio runtime is created, we will also see the `block_on` kind task.
use std::time::Duration;
use tokio::{runtime, task};

fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    console_subscriber::init();

    let rt = runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let local = task::LocalSet::new();
    local.block_on(&rt, async {
        loop {
            let mut join_handles = Vec::new();
            for _ in 0..10 {
                let jh = task::spawn_local(async {
                    tokio::time::sleep(Duration::from_millis(100)).await;

                    std::thread::sleep(Duration::from_millis(100));
                });
                join_handles.push(jh);
            }

            for jh in join_handles {
                _ = jh.await;
            }
        }
    });

    Ok(())
}
