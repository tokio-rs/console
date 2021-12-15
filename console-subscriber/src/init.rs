use crate::{Builder, TasksLayer};
use std::thread;
use tokio::runtime;
use tracing_subscriber::{
    filter::{FilterFn, Filtered, LevelFilter, Targets},
    prelude::*,
    Registry,
};

type ConsoleLayer = Filtered<TasksLayer, FilterFn, Registry>;

/// Initializes the console [tracing `Subscriber`][sub] and starts the console
/// subscriber [`Server`] on its own background thread.
///
/// This function represents the easiest way to get started using
/// tokio-console.
///
/// In addition to the [`TasksLayer`], which collects instrumentation data
/// consumed by the console, the default [`Subscriber`][sub] initialized by this
/// function also includes a [`tracing_subscriber::fmt`] layer, which logs
/// tracing spans and events to stdout. Which spans and events are logged will
/// be determined by the `RUST_LOG` environment variable.
///
/// **Note**: this function sets the [default `tracing` subscriber][default]
/// for your application. If you need to add additional layers to this
/// subscriber, see [`build`].
///
/// [default]: https://docs.rs/tracing/latest/tracing/dispatcher/index.html#setting-the-default-subscriber
/// [sub]: https://docs.rs/tracing/latest/tracing/trait.Subscriber.html
/// [`tracing_subscriber::fmt`]: https://docs.rs/tracing-subscriber/latest/tracing-subscriber/fmt/index.html
/// [`Server`]: crate::Server
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
/// | `TOKIO_CONSOLE_RECORD_PATH`         | The file path to save a recording                                         | None              |
/// | `RUST_LOG`                          | Configures what events are logged events. See [`Targets`] for details.    | "error"           |
///
/// ## Further customization
///
/// To add additional layers or replace the format layer, replace
/// `console_subscriber::init` with:
///
/// ```rust
/// use tracing_subscriber::prelude::*;
///
/// let console_layer = console_subscriber::build();
///
/// tracing_subscriber::registry()
///     .with(console_layer)
///     .with(tracing_subscriber::fmt::layer())
/// //  .with(..potential additional layer..)
///     .init();
/// ```
///
/// [`Targets`]: https://docs.rs/tracing-subscriber/latest/tracing-subscriber/filter/struct.Targets.html
pub fn init() {
    TasksLayer::builder().with_default_env().init();
}

/// Returns a new `tracing_subscriber` [`Layer`] configured with a [`TasksLayer`]
/// and a [filter] that enables the spans and events required by the console.
///
/// This function spawns the console subscriber's [`Server`] in its own Tokio
/// runtime in a background thread.
///
/// Unlike [`init`], this function does not set the default subscriber, allowing
/// additional [`Layer`]s to be added.
///
/// [filter]: https://docs.rs/tracing-subscriber/latest/tracing_subscriber/filter/struct.Targets.html
/// [`Layer`]: https://docs.rs/tracing-subscriber/latest/tracing_subscriber/layer/trait.Layer.html
/// [`Server`]: crate::Server
///
/// ## Configuration
///
/// `console_subscriber::build` supports all of the environmental
/// configuration described at [`console_subscriber::init`].
///
/// ## Differences from `init`
///
/// Unlike [`console_subscriber::init`], this function does *not* add a
/// [`tracing_subscriber::fmt`] layer to the configured `Layer`. This means
/// that this function will not log spans and events based on the value of the
/// `RUST_LOG` environment variable. Instead, a user-provided [`fmt::Layer`] can
/// be added in order to customize the log format.
///
/// You must call [`.init()`] on the final subscriber in order to [set the
/// subscriber as the default][default].
///
/// ## Examples
///
/// ```rust
/// use tracing_subscriber::prelude::*;
/// tracing_subscriber::registry()
///     .with(console_subscriber::build())
///     .with(tracing_subscriber::fmt::layer())
/// //  .with(...)
///     .init();
/// ```
/// [`.init()`]: https://docs.rs/tracing-subscriber/latest/tracing_subscriber/util/trait.SubscriberInitExt.html
/// [default]: https://docs.rs/tracing/latest/tracing/dispatcher/index.html#setting-the-default-subscriber
/// [sub]: https://docs.rs/tracing/latest/tracing/trait.Subscriber.html
/// [`tracing_subscriber::fmt`]: https://docs.rs/tracing-subscriber/latest/tracing-subscriber/fmt/index.html
/// [`fmt::Layer`]: https://docs.rs/tracing-subscriber/latest/tracing-subscriber/fmt/struct.Layer.html
/// [`console_subscriber::init`]: crate::init()
#[must_use = "build() without init() will not set the default tracing subscriber"]
pub fn build() -> ConsoleLayer {
    TasksLayer::builder()
        .with_default_env()
        .build_console_layer()
}

impl Builder {
    /// Initializes the console [tracing `Subscriber`][sub] and starts the console
    /// subscriber [`Server`] on its own background thread.
    ///
    /// This function represents the easiest way to get started using
    /// tokio-console.
    ///
    /// In addition to the [`TasksLayer`], which collects instrumentation data
    /// consumed by the console, the default [`Subscriber`][sub] initialized by this
    /// function also includes a [`tracing_subscriber::fmt`] layer, which logs
    /// tracing spans and events to stdout. Which spans and events are logged will
    /// be determined by the `RUST_LOG` environment variable.
    ///
    /// **Note**: this function sets the [default `tracing` subscriber][default]
    /// for your application. If you need to add additional layers to this
    /// subscriber, see [`build`].
    ///
    /// [default]: https://docs.rs/tracing/latest/tracing/dispatcher/index.html#setting-the-default-subscriber
    /// [sub]: https://docs.rs/tracing/latest/tracing/trait.Subscriber.html
    /// [`tracing_subscriber::fmt`]: https://docs.rs/tracing-subscriber/latest/tracing-subscriber/fmt/index.html
    /// [`Server`]: crate::Server
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
    /// | `TOKIO_CONSOLE_RECORD_PATH`         | The file path to save a recording                                         | None              |
    /// | `RUST_LOG`                          | Configures what events are logged events. See [`Targets`] for details.    | "error"           |
    ///
    /// ## Further customization
    ///
    /// To add additional layers or replace the format layer, replace
    /// `console_subscriber::Builder::init` with:
    ///
    /// ```rust
    /// use tracing_subscriber::prelude::*;
    ///
    /// let console_layer = console_subscriber::TasksLayer::builder().build_console_layer();
    ///
    /// tracing_subscriber::registry()
    ///     .with(console_layer)
    ///     .with(tracing_subscriber::fmt::layer())
    /// //  .with(..potential additional layer..)
    ///     .init();
    /// ```
    ///
    /// [`Targets`]: https://docs.rs/tracing-subscriber/latest/tracing-subscriber/filter/struct.Targets.html
    pub fn init(self) {
        let fmt_filter = std::env::var("RUST_LOG")
            .ok()
            .and_then(|rust_log| match rust_log.parse::<Targets>() {
                Ok(targets) => Some(targets),
                Err(e) => {
                    eprintln!("failed to parse `RUST_LOG={:?}`: {}", rust_log, e);
                    None
                }
            })
            .unwrap_or_else(|| Targets::default().with_default(LevelFilter::ERROR));

        let console_layer = self.build_console_layer();

        tracing_subscriber::registry()
            .with(console_layer)
            .with(tracing_subscriber::fmt::layer().with_filter(fmt_filter))
            .init();
    }

    /// Returns a new `tracing` [subscriber] configured with a [`TasksLayer`]
    /// and a [filter] that enables the spans and events required by the console.
    ///
    /// This function spawns the console subscriber's [`Server`] in its own Tokio
    /// runtime in a background thread.
    ///
    /// Unlike [`init`], this function does not set the default subscriber, allowing
    /// additional [`Layer`]s to be added.
    ///
    /// [subscriber]: https://docs.rs/tracing/latest/tracing/subscriber/trait.Subscriber.html
    /// [filter]: https://docs.rs/tracing-subscriber/latest/tracing_subscriber/filter/struct.Targets.html
    /// [`Layer`]: https://docs.rs/tracing-subscriber/latest/tracing_subscriber/layer/trait.Layer.html
    /// [`Server`]: crate::Server
    ///
    /// ## Configuration
    ///
    /// `console_subscriber::build` supports all of the environmental
    /// configuration described at [`console_subscriber::init`].
    ///
    /// ## Differences from `init`
    ///
    /// Unlike [`console_subscriber::init`], this function does *not* add a
    /// [`tracing_subscriber::fmt`] layer to the configured `Subscriber`. This means
    /// that this function will not log spans and events based on the value of the
    /// `RUST_LOG` environment variable. Instead, a user-provided [`fmt::Layer`] can
    /// be added in order to customize the log format.
    ///
    /// You must call [`.init()`] on the final subscriber in order to [set the
    /// subscriber as the default][default].
    ///
    /// ## Examples
    ///
    /// ```rust
    /// use tracing_subscriber::prelude::*;
    ///
    /// let console_layer = console_subscriber::TasksLayer::builder()
    ///     .with_default_env()
    ///     .build_console_layer();
    ///
    /// tracing_subscriber::registry()
    ///     .with(console_layer)
    ///     .with(tracing_subscriber::fmt::layer())
    /// //  .with(...)
    ///     .init();
    /// ```
    /// [`.init()`]: https://docs.rs/tracing-subscriber/latest/tracing_subscriber/util/trait.SubscriberInitExt.html
    /// [default]: https://docs.rs/tracing/latest/tracing/dispatcher/index.html#setting-the-default-subscriber
    /// [sub]: https://docs.rs/tracing/latest/tracing/trait.Subscriber.html
    /// [`tracing_subscriber::fmt`]: https://docs.rs/tracing-subscriber/latest/tracing-subscriber/fmt/index.html
    /// [`fmt::Layer`]: https://docs.rs/tracing-subscriber/latest/tracing-subscriber/fmt/struct.Layer.html
    /// [`console_subscriber::init`]: crate::init()
    #[must_use = "build_console_layer() without init() will not set the default tracing subscriber"]
    pub fn build_console_layer(self) -> ConsoleLayer {
        fn console_filter(meta: &tracing::Metadata<'_>) -> bool {
            // events will have *targets* beginning with "runtime"
            if meta.is_event() {
                return meta.target().starts_with("runtime") || meta.target().starts_with("tokio");
            }

            // spans will have *names* beginning with "runtime". for backwards
            // compatibility with older Tokio versions, enable anything with the `tokio`
            // target as well.
            meta.name().starts_with("runtime.") || meta.target().starts_with("tokio")
        }

        let (layer, server) = self.build();
        let filter =
            FilterFn::new(console_filter as for<'r, 's> fn(&'r tracing::Metadata<'s>) -> bool);
        let layer = layer.with_filter(filter);

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

        layer
    }
}
