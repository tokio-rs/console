use crate::TasksLayer;
use std::thread;
use tokio::runtime;
use tracing_subscriber::{fmt, layer::Layered, prelude::*, EnvFilter, Registry};

type ConsoleSubscriberLayer = Layered<TasksLayer, Layered<EnvFilter, Registry>>;

/// Starts the console subscriber server on its own thread
///
/// This function represents the easiest way to get started using
/// tokio-console.
///
/// **Note**: this function sets the [default `tracing` subscriber][default]
/// for your application. If you need to add additional layers to this
/// subscriber, see [`init_with_layer`].
///
/// [default]: https://docs.rs/tracing/0.1.26/tracing/dispatcher/index.html#setting-the-default-subscriber
///
/// ## Configuration
///
/// Tokio console subscriber is configured with sensible defaults for most
/// use cases. If you need to tune these parameters, several environmental
/// configuration variables are available:
///
/// | **Environment Variable**            | **Purpose**                                                               | **Default Value** |
/// |-------------------------------------|---------------------------------------------------------------------------|-------------------|
/// | `TOKIO_CONSOLE_RETENTION_SECS`      | The number of seconds to accumulate completed tracing data                | 3600s (1h)        |
/// | `TOKIO_CONSOLE_BIND`                | A HOST:PORT description, such as `localhost:1234`                         | `127.0.0.1:6669`  |
/// | `TOKIO_CONSOLE_PUBLISH_INTERVAL_MS` | The number of milliseconds to wait between sending updates to the console | 1000ms (1s)       |
/// | `RUST_LOG`                          | Configure the tracing filter. See [`EnvFilter`] for further information   | `tokio=trace`     |
///
/// ## Further customization
///
/// To add additional layers or replace the format layer, replace `console_subscriber::init` with:
///
/// ```rust
/// use tracing_subscriber::prelude::*;
/// console_subscriber::build()
///     .with(tracing_subscriber::fmt::layer())
/// //  .with(..potential additional layer..)
///     .init();
/// ```

pub fn init() {
    build().with(fmt::layer()).init()
}

/// Builds a new tracing subscriber, but does not yet initialize the
/// subscriber, allowing for customization by adding additional
/// layers.
///
/// ## Configuration
///
/// console_subscriber::build supports all of the environmental
/// configuration described at [`init`]
///
/// ## Differences from `init`
///
/// **note**: in order to support customizing the format `build` does
/// not attach a [`tracing_subscriber::fmt::layer`], whereas init does.
///
/// Additionally, you must call [`init`][tracing_subscriber::util::SubscriberInitExt::init] on the final layer in order to
/// register the subscriber.
///
/// ## Examples
///
/// ```rust
/// use tracing_subscriber::prelude::*;
/// console_subscriber::build()
///     .with(tracing_subscriber::fmt::layer())
/// //  .with(...)
///     .init();
/// ```

#[must_use = "build() without init() will not attach a tracing subscriber"]
pub fn build() -> ConsoleSubscriberLayer {
    let (layer, server) = TasksLayer::builder().from_default_env().build();

    let filter = EnvFilter::from_default_env().add_directive("tokio=trace".parse().unwrap());

    let console_subscriber = tracing_subscriber::registry().with(filter).with(layer);

    thread::Builder::new()
        .name("console_subscriber".into())
        .spawn(move || {
            let runtime = runtime::Builder::new_current_thread()
                .enable_io()
                .enable_time()
                .build()
                .expect("console subscriber runtime initialization failed");

            runtime.block_on(async move {
                server
                    .serve()
                    .await
                    .expect("console subscriber server failed")
            });
        })
        .expect("console subscriber could not spawn thread");

    console_subscriber
}
