//! Demonstrates serving the console API over a [Unix domain socket] (UDS)
//! connection, rather than over TCP.
//!
//! Note that this example only works on Unix operating systems that
//! support UDS, such as Linux, BSDs, and macOS.
//!
//! [Unix domain socket]: https://en.wikipedia.org/wiki/Unix_domain_socket

#[cfg(unix)]
use {
    std::time::Duration,
    tokio::{fs, task, time},
    tracing::info,
};

#[cfg(unix)]
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let cwd = fs::canonicalize(".").await?;
    let addr = cwd.join("console-server");
    console_subscriber::ConsoleLayer::builder()
        .server_addr(&*addr)
        .init();
    info!(
        "listening for console connections at file://localhost{}",
        addr.display()
    );
    task::Builder::default()
        .name("sleepy")
        .spawn(async move { time::sleep(Duration::from_secs(90)).await })
        .unwrap()
        .await?;

    Ok(())
}

#[cfg(not(unix))]
fn main() {
    panic!("only supported on Unix platforms")
}
