use super::{DefaultFields, Server, TasksLayer};
use std::{net::SocketAddr, time::Duration};

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
    pub(super) publish_interval: Duration,

    /// How long to retain data for completed events.
    pub(super) retention: Duration,

    /// The address on which to serve the RPC server.
    pub(super) server_addr: SocketAddr,
}

impl Default for Builder {
    fn default() -> Self {
        Self {
            event_buffer_capacity: TasksLayer::<DefaultFields>::DEFAULT_EVENT_BUFFER_CAPACITY,
            client_buffer_capacity: TasksLayer::<DefaultFields>::DEFAULT_CLIENT_BUFFER_CAPACITY,
            publish_interval: TasksLayer::<DefaultFields>::DEFAULT_PUBLISH_INTERVAL,
            retention: TasksLayer::<DefaultFields>::DEFAULT_RETENTION,
            server_addr: SocketAddr::new(Server::DEFAULT_IP, Server::DEFAULT_PORT),
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

    /// Completes the builder, returning a [`TasksLayer`] and [`Server`] task.
    pub fn build(self) -> (TasksLayer, Server) {
        TasksLayer::build(self)
    }
}
