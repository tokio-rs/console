use super::{ConsoleLayer, Server};
#[cfg(unix)]
use std::path::Path;
use std::{
    net::{IpAddr, SocketAddr, SocketAddrV4, SocketAddrV6, ToSocketAddrs},
    path::PathBuf,
    thread,
    time::Duration,
};
use tokio::runtime;
use tracing::Subscriber;
use tracing_subscriber::{
    filter::{self, FilterFn},
    layer::{Layer, SubscriberExt},
    prelude::*,
    registry::LookupSpan,
};

/// Builder for configuring [`ConsoleLayer`]s.
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
    pub(super) server_addr: ServerAddr,

    /// If and where to save a recording of the events.
    pub(super) recording_path: Option<PathBuf>,

    /// The filter environment variable to use for `tracing` events.
    pub(super) filter_env_var: String,

    /// Whether to trace events coming from the subscriber thread
    self_trace: bool,

    /// The maximum value for the task poll duration histogram.
    ///
    /// Any polls exceeding this duration will be clamped to this value. Higher
    /// values will result in more memory usage.
    pub(super) poll_duration_max: Duration,

    /// The maximum value for the task scheduled duration histogram.
    ///
    /// Any scheduled times exceeding this duration will be clamped to this
    /// value. Higher values will result in more memory usage.
    pub(super) scheduled_duration_max: Duration,

    /// Whether to enable the grpc-web support.
    #[cfg(feature = "grpc-web")]
    enable_grpc_web: bool,
}

impl Default for Builder {
    fn default() -> Self {
        Self {
            event_buffer_capacity: ConsoleLayer::DEFAULT_EVENT_BUFFER_CAPACITY,
            client_buffer_capacity: ConsoleLayer::DEFAULT_CLIENT_BUFFER_CAPACITY,
            publish_interval: ConsoleLayer::DEFAULT_PUBLISH_INTERVAL,
            retention: ConsoleLayer::DEFAULT_RETENTION,
            poll_duration_max: ConsoleLayer::DEFAULT_POLL_DURATION_MAX,
            scheduled_duration_max: ConsoleLayer::DEFAULT_SCHEDULED_DURATION_MAX,
            server_addr: ServerAddr::Tcp(SocketAddr::new(Server::DEFAULT_IP, Server::DEFAULT_PORT)),
            recording_path: None,
            filter_env_var: "RUST_LOG".to_string(),
            self_trace: false,
            #[cfg(feature = "grpc-web")]
            enable_grpc_web: false,
        }
    }
}

impl Builder {
    /// Sets the maximum capacity for the channel of events sent from subscriber
    /// layers to the aggregator task.
    ///
    /// When this channel is at capacity, additional events will be dropped.
    ///
    /// By default, this is [`ConsoleLayer::DEFAULT_EVENT_BUFFER_CAPACITY`].
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
    /// By default, this is [`ConsoleLayer::DEFAULT_CLIENT_BUFFER_CAPACITY`].
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
    /// By default, this is [`ConsoleLayer::DEFAULT_PUBLISH_INTERVAL`].
    /// Methods like [`init`][`crate::init`] and [`spawn`][`crate::spawn`] will
    /// take the value from the `TOKIO_CONSOLE_PUBLISH_INTERVAL` [environment
    /// variable] before falling back on that default.
    ///
    /// [environment variable]: `Builder::with_default_env`
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
    /// By default, this is [`ConsoleLayer::DEFAULT_RETENTION`]. Methods
    /// like [`init`][`crate::init`] and [`spawn`][`crate::spawn`] will take the
    /// value from the `TOKIO_CONSOLE_RETENTION` [environment variable] before
    /// falling back on that default.
    ///
    /// [environment variable]: `Builder::with_default_env`
    pub fn retention(self, retention: Duration) -> Self {
        Self { retention, ..self }
    }

    /// Sets the socket address on which to serve the RPC server.
    ///
    /// By default, the server is bound on the IP address [`Server::DEFAULT_IP`]
    /// on port [`Server::DEFAULT_PORT`]. Methods like
    /// [`init`][`crate::init`] and [`spawn`][`crate::spawn`] will parse the
    /// socket address from the `TOKIO_CONSOLE_BIND` [environment variable]
    /// before falling back on constructing a socket address from those
    /// defaults.
    ///
    /// The socket address can be either a TCP socket address, a
    /// [Unix domain socket] (UDS) address, or a [Vsock] address.
    /// Unix domain sockets are only supported on Unix-compatible operating systems,
    /// such as Linux, BSDs, and macOS. Vsock addresses are only available when the
    /// "vsock" feature is enabled and are supported on platforms with vsock capability.
    ///
    /// Each call to this method will overwrite the previously set value.
    ///
    /// # Examples
    ///
    /// Connect to the TCP address `localhost:1234`:
    ///
    /// ```
    /// # use console_subscriber::Builder;
    /// use std::net::Ipv4Addr;
    /// let builder = Builder::default().server_addr((Ipv4Addr::LOCALHOST, 1234));
    /// ```
    ///
    /// Connect to the UDS address `/tmp/tokio-console`:
    ///
    /// ```
    /// # use console_subscriber::Builder;
    /// # #[cfg(unix)]
    /// use std::path::Path;
    ///
    /// // Unix domain sockets are only available on Unix-compatible operating systems.
    /// #[cfg(unix)]
    /// let builder = Builder::default().server_addr(Path::new("/tmp/tokio-console"));
    /// ```
    ///
    /// Connect using a vsock connection (requires the "vsock" feature):
    ///
    /// ```
    /// # use console_subscriber::Builder;
    /// # #[cfg(feature = "vsock")]
    /// let builder = Builder::default().server_addr((tokio_vsock::VMADDR_CID_ANY, 6669));
    /// ```
    ///
    /// [environment variable]: `Builder::with_default_env`
    /// [Unix domain socket]: https://en.wikipedia.org/wiki/Unix_domain_socket
    /// [Vsock]: https://docs.rs/tokio-vsock/latest/tokio_vsock/
    pub fn server_addr(self, server_addr: impl Into<ServerAddr>) -> Self {
        Self {
            server_addr: server_addr.into(),
            ..self
        }
    }

    /// Sets the path to record the events to the file system.
    ///
    /// By default, this is initially `None`. Methods like
    /// [`init`][`crate::init`] and [`spawn`][`crate::spawn`] will take the
    /// value from the `TOKIO_CONSOLE_RECORD_PATH` [environment variable] before
    /// falling back on that default.
    ///
    /// [environment variable]: `Builder::with_default_env`
    pub fn recording_path(self, path: impl Into<PathBuf>) -> Self {
        Self {
            recording_path: Some(path.into()),
            ..self
        }
    }

    /// Sets the environment variable used to configure which `tracing` events
    /// are logged to stdout.
    ///
    /// The [`Builder::init`] method configures the default `tracing`
    /// subscriber. In addition to a [`ConsoleLayer`], the subscriber
    /// constructed by `init` includes a [`fmt::Layer`] for logging events to
    /// stdout. What `tracing` events that layer will log is determined by the
    /// value of an environment variable; this method configures which
    /// environment variable is read to determine the log filter.
    ///
    /// This environment variable does not effect what spans and events are
    /// recorded by the [`ConsoleLayer`]. Therefore, this method will have no
    /// effect if the builder is used with [`Builder::spawn`] or
    /// [`Builder::build`].
    ///
    /// The default environment variable is `RUST_LOG`. See [here] for details
    /// on the syntax for configuring the filter.
    ///
    /// [`fmt::Layer`]: https://docs.rs/tracing-subscriber/0.3/tracing_subscriber/fmt/index.html
    /// [here]: https://docs.rs/tracing-subscriber/0.3/tracing_subscriber/filter/targets/struct.Targets.html
    pub fn filter_env_var(self, filter_env_var: impl Into<String>) -> Self {
        Self {
            filter_env_var: filter_env_var.into(),
            ..self
        }
    }

    /// Sets the maximum value for task poll duration histograms.
    ///
    /// Any poll durations exceeding this value will be clamped down to this
    /// duration and recorded as an outlier.
    ///
    /// By default, this is [one second]. Higher values will increase per-task
    /// memory usage.
    ///
    /// [one second]: ConsoleLayer::DEFAULT_POLL_DURATION_MAX
    pub fn poll_duration_histogram_max(self, max: Duration) -> Self {
        Self {
            poll_duration_max: max,
            ..self
        }
    }

    /// Sets the maximum value for task scheduled duration histograms.
    ///
    /// Any scheduled duration (the time from a task being woken until it is next
    /// polled) exceeding this value will be clamped down to this duration
    /// and recorded as an outlier.
    ///
    /// By default, this is [one second]. Higher values will increase per-task
    /// memory usage.
    ///
    /// [one second]: ConsoleLayer::DEFAULT_SCHEDULED_DURATION_MAX
    pub fn scheduled_duration_histogram_max(self, max: Duration) -> Self {
        Self {
            scheduled_duration_max: max,
            ..self
        }
    }

    /// Sets whether tasks, resources, and async ops from the console
    /// subscriber thread are recorded.
    ///
    /// By default, events from the console subscriber are discarded and
    /// not exported to clients.
    pub fn enable_self_trace(self, self_trace: bool) -> Self {
        Self { self_trace, ..self }
    }

    /// Sets whether to enable the grpc-web support.
    ///
    /// By default, this is `false`. If enabled, the console subscriber will
    /// serve the gRPC-Web protocol in addition to the standard gRPC protocol.
    /// This is useful for serving the console subscriber to web clients.
    /// Please be aware that the current default server port is set to 6669.
    /// However, certain browsers may restrict this port due to security reasons.
    /// If you encounter issues with this, consider changing the port to an
    /// alternative one that is not commonly blocked by browsers.
    ///
    /// [`serve_with_grpc_web`] is used to provide more advanced configuration
    /// for the gRPC-Web server.
    ///
    /// [`serve_with_grpc_web`]: crate::Server::serve_with_grpc_web
    #[cfg(feature = "grpc-web")]
    pub fn enable_grpc_web(self, enable_grpc_web: bool) -> Self {
        Self {
            enable_grpc_web,
            ..self
        }
    }

    /// Completes the builder, returning a [`ConsoleLayer`] and [`Server`] task.
    pub fn build(self) -> (ConsoleLayer, Server) {
        ConsoleLayer::build(self)
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
            self.server_addr = ServerAddr::Tcp(
                bind.to_socket_addrs()
                    .expect(
                        "TOKIO_CONSOLE_BIND must be formatted as HOST:PORT, such as localhost:4321",
                    )
                    .next()
                    .expect("tokio console could not resolve TOKIO_CONSOLE_BIND"),
            );
        }

        if let Some(interval) = duration_from_env("TOKIO_CONSOLE_PUBLISH_INTERVAL") {
            self.publish_interval = interval;
        }

        if let Ok(path) = std::env::var("TOKIO_CONSOLE_RECORD_PATH") {
            self.recording_path = Some(path.into());
        }

        if let Some(capacity) = usize_from_env("TOKIO_CONSOLE_BUFFER_CAPACITY") {
            self.event_buffer_capacity = capacity;
        }

        self
    }

    /// Initializes the console [tracing `Subscriber`][sub] and starts the console
    /// subscriber [`Server`] on its own background thread.
    ///
    /// This function represents the easiest way to get started using
    /// tokio-console.
    ///
    /// In addition to the [`ConsoleLayer`], which collects instrumentation data
    /// consumed by the console, the default [`Subscriber`][sub] initialized by this
    /// function also includes a [`tracing_subscriber::fmt`] layer, which logs
    /// tracing spans and events to stdout. Which spans and events are logged will
    /// be determined by an environment variable, which defaults to `RUST_LOG`.
    /// The [`Builder::filter_env_var`] method can be used to override the
    /// environment variable used to configure the log filter.
    ///
    /// **Note**: this function sets the [default `tracing` subscriber][default]
    /// for your application. If you need to add additional layers to a subscriber,
    /// see [`spawn`].
    ///
    /// # Panics
    ///
    /// * If the subscriber's background thread could not be spawned.
    /// * If the [default `tracing` subscriber][default] has already been set.
    ///
    /// [default]: https://docs.rs/tracing/latest/tracing/dispatcher/index.html#setting-the-default-subscriber
    /// [sub]: https://docs.rs/tracing/latest/tracing/trait.Subscriber.html
    /// [`tracing_subscriber::fmt`]: https://docs.rs/tracing-subscriber/latest/tracing-subscriber/fmt/index.html
    /// [`Server`]: crate::Server
    ///
    /// # Configuration
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
    /// If the "env-filter" crate feature flag is enabled, the `RUST_LOG`
    /// environment variable will be parsed using the [`EnvFilter`] type from
    /// `tracing-subscriber`. If the "env-filter" feature is **not** enabled, the
    /// [`Targets`] filter is used instead. The `EnvFilter` type accepts all the
    /// same syntax as `Targets`, but with the added ability to filter dynamically
    /// on span field values. See the documentation for those types for details.
    ///
    /// # Further customization
    ///
    /// To add additional layers or replace the format layer, replace
    /// `console_subscriber::Builder::init` with:
    ///
    /// ```rust
    /// use tracing_subscriber::prelude::*;
    ///
    /// let console_layer = console_subscriber::ConsoleLayer::builder().spawn();
    ///
    /// tracing_subscriber::registry()
    ///     .with(console_layer)
    ///     .with(tracing_subscriber::fmt::layer())
    /// //  .with(..potential additional layer..)
    ///     .init();
    /// ```
    ///
    /// [`Targets`]: https://docs.rs/tracing-subscriber/latest/tracing-subscriber/filter/struct.Targets.html
    /// [`EnvFilter`]: https://docs.rs/tracing-subscriber/latest/tracing-subscriber/filter/struct.EnvFilter.html
    pub fn init(self) {
        #[cfg(feature = "env-filter")]
        type Filter = filter::EnvFilter;
        #[cfg(not(feature = "env-filter"))]
        type Filter = filter::Targets;

        let fmt_filter = std::env::var(&self.filter_env_var)
            .ok()
            .and_then(|log_filter| match log_filter.parse::<Filter>() {
                Ok(targets) => Some(targets),
                Err(e) => {
                    eprintln!(
                        "failed to parse filter environment variable `{}={:?}`: {}",
                        &self.filter_env_var, log_filter, e
                    );
                    None
                }
            })
            .unwrap_or_else(|| {
                "error"
                    .parse::<Filter>()
                    .expect("`error` filter should always parse successfully")
            });

        let console_layer = self.spawn();

        tracing_subscriber::registry()
            .with(console_layer)
            .with(tracing_subscriber::fmt::layer().with_filter(fmt_filter))
            .init();
    }

    /// Returns a new `tracing` [`Layer`] consisting of a [`ConsoleLayer`]
    /// and a [filter] that enables the spans and events required by the console.
    ///
    /// This function spawns the console subscriber's [`Server`] in its own Tokio
    /// runtime in a background thread.
    ///
    /// Unlike [`init`], this function does not set the default subscriber, allowing
    /// additional [`Layer`]s to be added.
    ///
    /// [subscriber]: https://docs.rs/tracing/latest/tracing/subscriber/trait.Subscriber.html
    /// [filter]: https://docs.rs/tracing-subscriber/latest/tracing_subscriber/filter/targets/struct.Targets.html
    /// [`Layer`]: https://docs.rs/tracing-subscriber/latest/tracing_subscriber/layer/trait.Layer.html
    /// [`Server`]: crate::Server
    ///
    /// # Panics
    ///
    /// * If the subscriber's background thread could not be spawned.
    ///
    /// # Configuration
    ///
    /// `console_subscriber::build` supports all of the environmental
    /// configuration described at [`console_subscriber::init`].
    ///
    /// # Differences from `init`
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
    /// # Examples
    ///
    /// ```rust
    /// use tracing_subscriber::prelude::*;
    ///
    /// let console_layer = console_subscriber::ConsoleLayer::builder()
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
    pub fn spawn<S>(self) -> impl Layer<S>
    where
        S: Subscriber + for<'a> LookupSpan<'a>,
    {
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

        let self_trace = self.self_trace;
        #[cfg(feature = "grpc-web")]
        let enable_grpc_web = self.enable_grpc_web;

        let (layer, server) = self.build();
        let filter =
            FilterFn::new(console_filter as for<'r, 's> fn(&'r tracing::Metadata<'s>) -> bool);
        let layer = layer.with_filter(filter);

        thread::Builder::new()
            .name("console_subscriber".into())
            .spawn(move || {
                let _subscriber_guard;
                if !self_trace {
                    _subscriber_guard = tracing::subscriber::set_default(
                        tracing_core::subscriber::NoSubscriber::default(),
                    );
                }
                let runtime = runtime::Builder::new_current_thread()
                    .enable_io()
                    .enable_time()
                    .build()
                    .expect("console subscriber runtime initialization failed");
                runtime.block_on(async move {
                    #[cfg(feature = "grpc-web")]
                    if enable_grpc_web {
                        server
                            .serve_with_grpc_web(tonic::transport::Server::builder())
                            .await
                            .expect("console subscriber server failed");
                        return;
                    }

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

/// Specifies the address on which a [`Server`] should listen.
///
/// This type is passed as an argument to the [`Builder::server_addr`]
/// method, and may be either a TCP socket address, or a [Unix domain socket]
/// (UDS) address. Unix domain sockets are only supported on Unix-compatible
/// operating systems, such as Linux, BSDs, and macOS.
///
/// [`Server`]: crate::Server
/// [Unix domain socket]: https://en.wikipedia.org/wiki/Unix_domain_socket
#[derive(Clone, Debug)]
#[non_exhaustive]
pub enum ServerAddr {
    /// A TCP address.
    Tcp(SocketAddr),
    /// A Unix socket address.
    #[cfg(unix)]
    Unix(PathBuf),
    /// A vsock address.
    #[cfg(feature = "vsock")]
    Vsock(tokio_vsock::VsockAddr),
}

impl From<SocketAddr> for ServerAddr {
    fn from(addr: SocketAddr) -> ServerAddr {
        ServerAddr::Tcp(addr)
    }
}

impl From<SocketAddrV4> for ServerAddr {
    fn from(addr: SocketAddrV4) -> ServerAddr {
        ServerAddr::Tcp(addr.into())
    }
}

impl From<SocketAddrV6> for ServerAddr {
    fn from(addr: SocketAddrV6) -> ServerAddr {
        ServerAddr::Tcp(addr.into())
    }
}

impl<I> From<(I, u16)> for ServerAddr
where
    I: Into<IpAddr>,
{
    fn from(pieces: (I, u16)) -> ServerAddr {
        ServerAddr::Tcp(pieces.into())
    }
}

#[cfg(unix)]
impl From<PathBuf> for ServerAddr {
    fn from(path: PathBuf) -> ServerAddr {
        ServerAddr::Unix(path)
    }
}

#[cfg(unix)]
impl<'a> From<&'a Path> for ServerAddr {
    fn from(path: &'a Path) -> ServerAddr {
        ServerAddr::Unix(path.to_path_buf())
    }
}

#[cfg(feature = "vsock")]
impl From<tokio_vsock::VsockAddr> for ServerAddr {
    fn from(addr: tokio_vsock::VsockAddr) -> ServerAddr {
        ServerAddr::Vsock(addr)
    }
}

/// Initializes the console [tracing `Subscriber`][sub] and starts the console
/// subscriber [`Server`] on its own background thread.
///
/// This function represents the easiest way to get started using
/// tokio-console.
///
/// In addition to the [`ConsoleLayer`], which collects instrumentation data
/// consumed by the console, the default [`Subscriber`][sub] initialized by this
/// function also includes a [`tracing_subscriber::fmt`] layer, which logs
/// tracing spans and events to stdout. Which spans and events are logged will
/// be determined by the `RUST_LOG` environment variable.
///
/// **Note**: this function sets the [default `tracing` subscriber][default]
/// for your application. If you need to add additional layers to a subscriber,
/// see [`spawn`].
///
/// # Panics
///
/// * If the subscriber's background thread could not be spawned.
/// * If the [default `tracing` subscriber][default] has already been set.
///
/// [default]: https://docs.rs/tracing/latest/tracing/dispatcher/index.html#setting-the-default-subscriber
/// [sub]: https://docs.rs/tracing/latest/tracing/trait.Subscriber.html
/// [`tracing_subscriber::fmt`]: https://docs.rs/tracing-subscriber/latest/tracing-subscriber/fmt/index.html
/// [`Server`]: crate::Server
///
/// # Configuration
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
/// If the "env-filter" crate feature flag is enabled, the `RUST_LOG`
/// environment variable will be parsed using the [`EnvFilter`] type from
/// `tracing-subscriber`. If the "env-filter" feature is **not** enabled, the
/// [`Targets`] filter is used instead. The `EnvFilter` type accepts all the
/// same syntax as `Targets`, but with the added ability to filter dynamically
/// on span field values. See the documentation for those types for details.
///
/// # Further customization
///
/// To add additional layers or replace the format layer, replace
/// `console_subscriber::init` with:
///
/// ```rust
/// use tracing_subscriber::prelude::*;
///
/// let console_layer = console_subscriber::spawn();
///
/// tracing_subscriber::registry()
///     .with(console_layer)
///     .with(tracing_subscriber::fmt::layer())
/// //  .with(..potential additional layer..)
///     .init();
/// ```
///
/// Calling `console_subscriber::init` is equivalent to the following:
/// ```rust
/// use console_subscriber::ConsoleLayer;
///
/// ConsoleLayer::builder().with_default_env().init();
/// ```
///
/// [`Targets`]: https://docs.rs/tracing-subscriber/latest/tracing-subscriber/filter/struct.Targets.html
/// [`EnvFilter`]: https://docs.rs/tracing-subscriber/latest/tracing-subscriber/filter/struct.EnvFilter.html
pub fn init() {
    ConsoleLayer::builder().with_default_env().init();
}

/// Returns a new `tracing_subscriber` [`Layer`] configured with a [`ConsoleLayer`]
/// and a [filter] that enables the spans and events required by the console.
///
/// This function spawns the console subscriber's [`Server`] in its own Tokio
/// runtime in a background thread.
///
/// Unlike [`init`], this function does not set the default subscriber, allowing
/// additional [`Layer`]s to be added.
///
/// This function is equivalent to the following:
/// ```
/// use console_subscriber::ConsoleLayer;
///
/// let layer = ConsoleLayer::builder().with_default_env().spawn();
/// # use tracing_subscriber::prelude::*;
/// # tracing_subscriber::registry().with(layer).init(); // to suppress must_use warnings
/// ```
/// [filter]: https://docs.rs/tracing-subscriber/latest/tracing_subscriber/filter/targets/struct.Targets.html
/// [`Layer`]: https://docs.rs/tracing-subscriber/latest/tracing_subscriber/layer/trait.Layer.html
/// [`Server`]: crate::Server
///
/// # Panics
///
/// * If the subscriber's background thread could not be spawned.
///
/// # Configuration
///
/// `console_subscriber::build` supports all of the environmental
/// configuration described at [`console_subscriber::init`].
///
/// # Differences from `init`
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
/// # Examples
///
/// ```rust
/// use tracing_subscriber::prelude::*;
/// tracing_subscriber::registry()
///     .with(console_subscriber::spawn())
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
#[must_use = "a `Layer` must be added to a `tracing::Subscriber`in order to be used"]
pub fn spawn<S>() -> impl Layer<S>
where
    S: Subscriber + for<'a> LookupSpan<'a>,
{
    ConsoleLayer::builder().with_default_env().spawn::<S>()
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

fn usize_from_env(var_name: &str) -> Option<usize> {
    let var = std::env::var(var_name).ok()?;
    match var.parse::<usize>() {
        Ok(num) => Some(num),
        Err(e) => panic!(
            "failed to parse a usize from `{}={:?}`: {}",
            var_name, var, e
        ),
    }
}
