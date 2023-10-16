use std::time::Duration;
use tokio::{runtime::Runtime, task};

fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    console_subscriber::init();

    let rt = Runtime::new().unwrap();
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
