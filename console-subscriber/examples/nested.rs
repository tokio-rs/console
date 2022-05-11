use tokio::task;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    use tracing_subscriber::prelude::*;

    tracing_subscriber::registry()
        .with(console_subscriber::spawn())
        .init();

    task::Builder::default()
        .name("main-task")
        .spawn(async move {
            foo().await;
        })
        .await?;

    Ok(())
}

#[tracing::instrument]
async fn foo() {
    println!("{}", tracing::Span::current().metadata().unwrap().name());
    bar().await
}

#[tracing::instrument]
async fn bar() {
    println!("  {}", tracing::Span::current().metadata().unwrap().name());
    baz().await
}

#[tracing::instrument]
async fn baz() {
    println!(
        "    {}",
        tracing::Span::current().metadata().unwrap().name()
    );
    loop {
        task::yield_now().await;
    }
}
