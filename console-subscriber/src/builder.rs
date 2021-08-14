use super::{Server, TasksLayer};
use std::{
    net::{SocketAddr, ToSocketAddrs},
    path::PathBuf,
    time::Duration,
};

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
    /// | **Environment Variable**            | **Purpose**                                                               | **Default Value** |
    /// |-------------------------------------|---------------------------------------------------------------------------|-------------------|
    /// | `TOKIO_CONSOLE_RETENTION_SECS`      | The number of seconds to accumulate completed tracing data                | 3600s (1h)        |
    /// | `TOKIO_CONSOLE_BIND`                | a HOST:PORT description, such as `localhost:1234`                         | `127.0.0.1:6669`  |
    /// | `TOKIO_CONSOLE_PUBLISH_INTERVAL_MS` | The number of milliseconds to wait between sending updates to the console | 1000ms (1s)       |
    /// | `TOKIO_CONSOLE_RECORD_PATH`         | The file path to save a recording                                         | None              |
    pub fn with_default_env(mut self) -> Self {
        if let Ok(retention) = std::env::var("TOKIO_CONSOLE_RETENTION_SECS") {
            self.retention = Duration::from_secs(
                retention
                    .parse()
                    .expect("TOKIO_CONSOLE_RETENTION_SECS must be an integer"),
            );
        }

        if let Ok(bind) = std::env::var("TOKIO_CONSOLE_BIND") {
            self.server_addr = bind
                .to_socket_addrs()
                .expect("TOKIO_CONSOLE_BIND must be formatted as HOST:PORT, such as localhost:4321")
                .next()
                .expect("tokio console could not resolve TOKIO_CONSOLE_BIND");
        }

        if let Ok(interval) = std::env::var("TOKIO_CONSOLE_PUBLISH_INTERVAL_MS") {
            self.publish_interval = Duration::from_millis(
                interval
                    .parse()
                    .expect("TOKIO_CONSOLE_PUBLISH_INTERVAL_MS must be an integer"),
            );
        }

        if let Ok(path) = std::env::var("TOKIO_CONSOLE_RECORD_PATH") {
            self.recording_path = Some(path.into());
        }

        self
    }
}
