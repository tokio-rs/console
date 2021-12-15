use super::{Server, TasksLayer};
use std::{
    net::{SocketAddr, ToSocketAddrs},
    path::PathBuf,
    thread,
    time::Duration,
};
use tokio::runtime;
use tracing_subscriber::{
    filter::{FilterFn, Filtered, LevelFilter, Targets},
    layer::SubscriberExt,
    prelude::*,
    Layer, Registry,
};

type ConsoleLayer = Filtered<TasksLayer, FilterFn, Registry>;

/// Builder for configuring [`TasksLayer`]s.
#[derive(Clone, Debug)]
pub struct Builder {
    /// The maximum capacity for the channel of events from the subscriber to
    /// the aggregator task.
    pub(super) event_buffer_capacity: usize,

    /// The maximum number of updates to buffer per-client before the client is
    /// dropped.
    pub(super) client_buffer_capacity: usize,

    /// The interval between publishing updates to clients.
    pub(crate) publish_interval: Duration,

    /// How long to retain data for completed events.
    pub(crate) retention: Duration,

    /// The address on which to serve the RPC server.
    pub(super) server_addr: SocketAddr,

    /// If and where to save a recording of the events.
    pub(super) recording_path: Option<PathBuf>,
}

impl Default for Builder {
    fn default() -> Self {
        Self {
            event_buffer_capacity: TasksLayer::DEFAULT_EVENT_BUFFER_CAPACITY,
            client_buffer_capacity: TasksLayer::DEFAULT_CLIENT_BUFFER_CAPACITY,
            publish_interval: TasksLayer::DEFAULT_PUBLISH_INTERVAL,
            retention: TasksLayer::DEFAULT_RETENTION,
            server_addr: SocketAddr::new(Server::DEFAULT_IP, Server::DEFAULT_PORT),
            recording_path: None,
        }
    }
}

impl Builder {
    /// Sets the maximum capacity for the channel of events sent from subscriber
    /// layers to the aggregator task.
    ///
    /// When this channel is at capacity, additional events will be dropped.
    ///
    /// By default, this is [`TasksLayer::DEFAULT_EVENT_BUFFER_CAPACITY`].
    pub fn event_buffer_capacity(self, event_buffer_capacity: usize) -> Self {
        Self {
            event_buffer_capacity,
            ..self
        }
    }

    /// Sets the maximum capacity of updates to buffer for each subscribed
    /// client, if that client is not reading from the RPC stream.
    ///
    /// When this channel is at capacity, the client may be disconnected.
    ///
    /// By default, this is [`TasksLayer::DEFAULT_CLIENT_BUFFER_CAPACITY`].
    pub fn client_buffer_capacity(self, client_buffer_capacity: usize) -> Self {
        Self {
            client_buffer_capacity,
            ..self
        }
    }

    /// Sets how frequently updates are published to clients.
    ///
    /// A shorter duration will allow clients to update more frequently, but may
    /// result in the program spending more time preparing task data updates.
    ///
    /// By default, this is [`TasksLayer::DEFAULT_PUBLISH_INTERVAL`].
    pub fn publish_interval(self, publish_interval: Duration) -> Self {
        Self {
            publish_interval,
            ..self
        }
    }

    /// Sets how long data is retained for completed tasks.
    ///
    /// A longer duration will allow more historical data to be replayed by
    /// clients, but will result in increased memory usage. A shorter duration
    /// will reduce memory usage, but less historical data from completed tasks
    /// will be retained.
    ///
    /// By default, this is [`TasksLayer::DEFAULT_RETENTION`].
    pub fn retention(self, retention: Duration) -> Self {
        Self { retention, ..self }
    }

    /// Sets the socket address on which to serve the RPC server.
    ///
    /// By default, the server is bound on the IP address [`Server::DEFAULT_IP`]
    /// on port [`Server::DEFAULT_PORT`].
    pub fn server_addr(self, server_addr: impl Into<SocketAddr>) -> Self {
        Self {
            server_addr: server_addr.into(),
            ..self
        }
    }

    /// Sets the path to record the events to the file system.
    pub fn recording_path(self, path: impl Into<PathBuf>) -> Self {
        Self {
            recording_path: Some(path.into()),
            ..self
        }
    }

    /// Completes the builder, returning a [`TasksLayer`] and [`Server`] task.
    pub fn build(self) -> (TasksLayer, Server) {
        TasksLayer::build(self)
    }

    /// Configures this builder from a standard set of environment variables:
    ///
    /// | **Environment Variable**         | **Purpose**                                                  | **Default Value** |
    /// |----------------------------------|--------------------------------------------------------------|-------------------|
    /// | `TOKIO_CONSOLE_RETENTION`        | The duration of seconds to accumulate completed tracing data | 3600s (1h)        |
    /// | `TOKIO_CONSOLE_BIND`             | a HOST:PORT description, such as `localhost:1234`            | `127.0.0.1:6669`  |
    /// | `TOKIO_CONSOLE_PUBLISH_INTERVAL` | The duration to wait between sending updates to the console  | 1000ms (1s)       |
    /// | `TOKIO_CONSOLE_RECORD_PATH`      | The file path to save a recording                            | None              |
    pub fn with_default_env(mut self) -> Self {
        if let Some(retention) = duration_from_env("TOKIO_CONSOLE_RETENTION") {
            self.retention = retention;
        }

        if let Ok(bind) = std::env::var("TOKIO_CONSOLE_BIND") {
            self.server_addr = bind
                .to_socket_addrs()
                .expect("TOKIO_CONSOLE_BIND must be formatted as HOST:PORT, such as localhost:4321")
                .next()
                .expect("tokio console could not resolve TOKIO_CONSOLE_BIND");
        }

        if let Some(interval) = duration_from_env("TOKIO_CONSOLE_PUBLISH_INTERVAL") {
            self.publish_interval = interval;
        }

        if let Ok(path) = std::env::var("TOKIO_CONSOLE_RECORD_PATH") {
            self.recording_path = Some(path.into());
        }

        self
    }

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
    /// for your application. If you need to add additional layers to a subscriber,
    /// see [`spawn`].
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
    /// | `TOKIO_CONSOLE_RETENTION`           | The number of seconds to accumulate completed tracing data                | 3600s (1h)        |
    /// | `TOKIO_CONSOLE_BIND`                | A HOST:PORT description, such as `localhost:1234`                         | `127.0.0.1:6669`  |
    /// | `TOKIO_CONSOLE_PUBLISH_INTERVAL`    | The number of milliseconds to wait between sending updates to the console | 1000ms (1s)       |
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
    /// let console_layer = console_subscriber::TasksLayer::builder().spawn();
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

        let console_layer = self.spawn();

        tracing_subscriber::registry()
            .with(console_layer)
            .with(tracing_subscriber::fmt::layer().with_filter(fmt_filter))
            .init();
    }

    /// Returns a new `tracing` [`Layer`] consisting of a [`TasksLayer`]
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
    ///     .spawn();
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
    #[must_use = "a `Layer` must be added to a `tracing::Subscriber` in order to be used"]
    pub fn spawn(self) -> ConsoleLayer {
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
/// for your application. If you need to add additional layers to a subscriber,
/// see [`spawn`].
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
/// | `TOKIO_CONSOLE_RETENTION`           | The number of seconds to accumulate completed tracing data                | 3600s (1h)        |
/// | `TOKIO_CONSOLE_BIND`                | A HOST:PORT description, such as `localhost:1234`                         | `127.0.0.1:6669`  |
/// | `TOKIO_CONSOLE_PUBLISH_INTERVAL`    | The number of milliseconds to wait between sending updates to the console | 1000ms (1s)       |
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
#[must_use = "spawn() without init() will not set the default tracing subscriber"]
pub fn spawn() -> ConsoleLayer {
    TasksLayer::builder().with_default_env().spawn()
}

fn duration_from_env(var_name: &str) -> Option<Duration> {
    let var = std::env::var(var_name).ok()?;
    match var.parse::<humantime::Duration>() {
        Ok(dur) => Some(dur.into()),
        Err(e) => panic!(
            "failed to parse a duration from `{}={:?}`: {}",
            var_name, var, e
        ),
    }
}
