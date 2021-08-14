use console_api as proto;
use serde::Serialize;
use tokio::sync::{mpsc, oneshot};

use std::{
    net::{IpAddr, Ipv4Addr, SocketAddr},
    sync::Arc,
    time::{Duration, SystemTime},
};
use tracing_core::{
    field::{self, Visit},
    span,
    subscriber::{self, Subscriber},
    Metadata,
};
use tracing_subscriber::{layer::Context, registry::LookupSpan, Layer};

mod aggregator;
mod builder;
mod callsites;
mod init;
mod record;

use aggregator::Aggregator;
pub use builder::Builder;
use callsites::Callsites;

pub use init::{build, init};

use crate::aggregator::TaskId;

pub struct TasksLayer {
    tx: mpsc::Sender<Event>,
    flush: Arc<aggregator::Flush>,
    spawn_callsites: Callsites,
    waker_callsites: Callsites,
}

pub struct Server {
    subscribe: mpsc::Sender<WatchKind>,
    addr: SocketAddr,
    aggregator: Option<Aggregator>,
    client_buffer: usize,
}

struct FieldVisitor {
    fields: Vec<proto::Field>,
    meta_id: proto::MetaId,
}

struct WakerVisitor {
    id: Option<span::Id>,
    op: Option<WakeOp>,
}

struct Watch<T>(mpsc::Sender<Result<T, tonic::Status>>);

enum WatchKind {
    Tasks(Watch<proto::tasks::TaskUpdate>),
    TaskDetail(WatchRequest<proto::tasks::TaskDetails>),
}

struct WatchRequest<T> {
    id: TaskId,
    stream_sender: oneshot::Sender<mpsc::Receiver<Result<T, tonic::Status>>>,
    buffer: usize,
}

enum Event {
    Metadata(&'static Metadata<'static>),
    Spawn {
        id: span::Id,
        metadata: &'static Metadata<'static>,
        at: SystemTime,
        fields: Vec<proto::Field>,
    },
    Enter {
        id: span::Id,
        at: SystemTime,
    },
    Exit {
        id: span::Id,
        at: SystemTime,
    },
    Close {
        id: span::Id,
        at: SystemTime,
    },
    Waker {
        id: span::Id,
        op: WakeOp,
        at: SystemTime,
    },
}

#[derive(Clone, Copy, Serialize)]
enum WakeOp {
    Wake,
    WakeByRef,
    Clone,
    Drop,
}

impl TasksLayer {
    pub fn new() -> (Self, Server) {
        Self::builder().build()
    }

    /// Returns a [`Builder`] for configuring a `TasksLayer`.
    pub fn builder() -> Builder {
        Builder::default()
    }

    fn build(config: Builder) -> (Self, Server) {
        // The `cfg` value *appears* to be a constant to clippy, but it changes
        // depending on the build-time configuration...
        #![allow(clippy::assertions_on_constants)]
        assert!(
            cfg!(tokio_unstable),
            "task tracing requires Tokio to be built with RUSTFLAGS=\"--cfg tokio_unstable\"!"
        );
        tracing::debug!(
            config.event_buffer_capacity,
            config.client_buffer_capacity,
            ?config.publish_interval,
            ?config.retention,
            ?config.server_addr,
            ?config.recording_path,
            "configured console subscriber"
        );

        let (tx, events) = mpsc::channel(config.event_buffer_capacity);
        let (subscribe, rpcs) = mpsc::channel(256);

        let aggregator = Aggregator::new(events, rpcs, &config);
        let flush = aggregator.flush().clone();

        let server = Server {
            aggregator: Some(aggregator),
            addr: config.server_addr,
            subscribe,
            client_buffer: config.client_buffer_capacity,
        };
        let layer = Self {
            tx,
            flush,
            spawn_callsites: Callsites::default(),
            waker_callsites: Callsites::default(),
        };
        (layer, server)
    }
}

impl TasksLayer {
    pub const DEFAULT_EVENT_BUFFER_CAPACITY: usize = 1024 * 10;
    pub const DEFAULT_CLIENT_BUFFER_CAPACITY: usize = 1024 * 4;
    pub const DEFAULT_PUBLISH_INTERVAL: Duration = Duration::from_secs(1);

    /// By default, completed spans are retained for one hour.
    pub const DEFAULT_RETENTION: Duration = Duration::from_secs(60 * 60);
    // how much capacity should remain in the buffer before triggering a
    // flush on capacity?
    //
    // chosen by fair die roll, guaranteed to be random :)
    const FLUSH_AT_CAPACITY: usize = 100;

    fn is_spawn(&self, meta: &'static Metadata<'static>) -> bool {
        self.spawn_callsites.contains(meta)
    }

    fn is_id_spawned<S>(&self, id: &span::Id, cx: &Context<'_, S>) -> bool
    where
        S: Subscriber + for<'a> LookupSpan<'a>,
    {
        cx.span(id)
            .map(|span| self.is_spawn(span.metadata()))
            .unwrap_or(false)
    }

    fn send(&self, event: Event) {
        use mpsc::error::TrySendError;

        match self.tx.try_reserve() {
            Ok(permit) => permit.send(event),
            Err(TrySendError::Closed(_)) => tracing::warn!(
                "console server task has terminated; task stats will no longer be updated"
            ),
            Err(TrySendError::Full(_)) => {
                // this shouldn't happen, since we trigger a flush when
                // approaching the high water line...but if the executor wait
                // time is very high, maybe the aggregator task hasn't been
                // polled yet. so, try to handle it gracefully...
                tracing::warn!(
                    "console buffer is full; some task stats may be delayed. \
                     preemptive flush interval should be adjusted..."
                );
                let tx = self.tx.clone();
                tokio::spawn(async move {
                    if tx.send(event).await.is_err() {
                        tracing::debug!("task event channel closed after lag");
                    }
                });
            }
        }

        let capacity = self.tx.capacity();
        if capacity <= Self::FLUSH_AT_CAPACITY {
            tracing::trace!(
                flush_at = Self::FLUSH_AT_CAPACITY,
                capacity,
                "at flush capacity..."
            );
            self.flush.trigger();
        }
    }
}

impl<S> Layer<S> for TasksLayer
where
    S: Subscriber + for<'a> LookupSpan<'a>,
{
    fn register_callsite(&self, meta: &'static Metadata<'static>) -> subscriber::Interest {
        if meta.name() == "runtime.spawn"
            // back compat until tokio is updated to use the standardized naming
            // scheme
            || (meta.name() == "task" && meta.target() == "tokio::task")
        {
            self.spawn_callsites.insert(meta);
        } else if meta.target() == "runtime::waker"
            // back compat until tokio is updated to use the standardized naming
            // scheme
            || meta.target() == "tokio::task::waker"
        {
            self.waker_callsites.insert(meta);
        }
        self.send(Event::Metadata(meta));

        subscriber::Interest::always()
    }

    fn new_span(&self, attrs: &span::Attributes<'_>, id: &span::Id, _: Context<'_, S>) {
        let metadata = attrs.metadata();
        if self.is_spawn(metadata) {
            let at = SystemTime::now();
            let mut fields_collector = FieldVisitor {
                fields: Vec::default(),
                meta_id: metadata.into(),
            };
            attrs.record(&mut fields_collector);

            self.send(Event::Spawn {
                id: id.clone(),
                at,
                metadata,
                fields: fields_collector.fields,
            });
        }
    }

    fn on_event(&self, event: &tracing::Event<'_>, _ctx: Context<'_, S>) {
        if self.waker_callsites.contains(event.metadata()) {
            let at = SystemTime::now();
            let mut visitor = WakerVisitor { id: None, op: None };
            event.record(&mut visitor);

            match visitor {
                WakerVisitor {
                    id: Some(id),
                    op: Some(op),
                } => {
                    self.send(Event::Waker { id, op, at });
                }
                _ => {
                    tracing::warn!("unknown waker event: {:?}", event);
                }
            }
        }
    }

    fn on_enter(&self, id: &span::Id, cx: Context<'_, S>) {
        if !self.is_id_spawned(id, &cx) {
            return;
        }
        self.send(Event::Enter {
            at: SystemTime::now(),
            id: id.clone(),
        });
    }

    fn on_exit(&self, id: &span::Id, cx: Context<'_, S>) {
        if !self.is_id_spawned(id, &cx) {
            return;
        }
        self.send(Event::Exit {
            at: SystemTime::now(),
            id: id.clone(),
        });
    }

    fn on_close(&self, id: span::Id, cx: Context<'_, S>) {
        if !self.is_id_spawned(&id, &cx) {
            return;
        }
        self.send(Event::Close {
            at: SystemTime::now(),
            id,
        });
    }
}

impl Server {
    // XXX(eliza): why is `SocketAddr::new` not `const`???
    pub const DEFAULT_IP: IpAddr = IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1));
    pub const DEFAULT_PORT: u16 = 6669;

    pub async fn serve(self) -> Result<(), Box<dyn std::error::Error + Send + Sync + 'static>> {
        self.serve_with(tonic::transport::Server::default()).await
    }

    pub async fn serve_with(
        mut self,
        mut builder: tonic::transport::Server,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync + 'static>> {
        let aggregate = self
            .aggregator
            .take()
            .expect("cannot start server multiple times");
        let aggregate = tokio::spawn(aggregate.run());
        let addr = self.addr;
        let res = builder
            .add_service(proto::tasks::tasks_server::TasksServer::new(self))
            .serve(addr)
            .await;
        aggregate.abort();
        res.map_err(Into::into)
    }
}

#[tonic::async_trait]
impl proto::tasks::tasks_server::Tasks for Server {
    type WatchTasksStream =
        tokio_stream::wrappers::ReceiverStream<Result<proto::tasks::TaskUpdate, tonic::Status>>;
    type WatchTaskDetailsStream =
        tokio_stream::wrappers::ReceiverStream<Result<proto::tasks::TaskDetails, tonic::Status>>;
    async fn watch_tasks(
        &self,
        req: tonic::Request<proto::tasks::TasksRequest>,
    ) -> Result<tonic::Response<Self::WatchTasksStream>, tonic::Status> {
        match req.remote_addr() {
            Some(addr) => tracing::debug!(client.addr = %addr, "starting a new watch"),
            None => tracing::debug!(client.addr = %"<unknown>", "starting a new watch"),
        }
        let permit = self.subscribe.reserve().await.map_err(|_| {
            tonic::Status::internal("cannot start new watch, aggregation task is not running")
        })?;
        let (tx, rx) = mpsc::channel(self.client_buffer);
        permit.send(WatchKind::Tasks(Watch(tx)));
        tracing::debug!("watch started");
        let stream = tokio_stream::wrappers::ReceiverStream::new(rx);
        Ok(tonic::Response::new(stream))
    }

    async fn watch_task_details(
        &self,
        req: tonic::Request<proto::tasks::DetailsRequest>,
    ) -> Result<tonic::Response<Self::WatchTaskDetailsStream>, tonic::Status> {
        let task_id = req
            .into_inner()
            .id
            .ok_or_else(|| tonic::Status::invalid_argument("missing task_id"))?;
        let permit = self.subscribe.reserve().await.map_err(|_| {
            tonic::Status::internal("cannot start new watch, aggregation task is not running")
        })?;

        // Check with the aggregator task to request a stream if the task exists.
        let (stream_sender, stream_recv) = oneshot::channel();
        permit.send(WatchKind::TaskDetail(WatchRequest {
            id: task_id.into(),
            stream_sender,
            buffer: self.client_buffer,
        }));
        // If the aggregator drops the sender, the task doesn't exist.
        let rx = stream_recv.await.map_err(|_| {
            tracing::warn!(id = ?task_id, "requested task not found");
            tonic::Status::not_found("task not found")
        })?;

        tracing::debug!(id = ?task_id, "task details watch started");
        let stream = tokio_stream::wrappers::ReceiverStream::new(rx);
        Ok(tonic::Response::new(stream))
    }
}

impl Visit for FieldVisitor {
    fn record_debug(&mut self, field: &field::Field, value: &dyn std::fmt::Debug) {
        self.fields.push(proto::Field {
            name: Some(field.name().into()),
            value: Some(value.into()),
            metadata_id: Some(self.meta_id.clone()),
        });
    }

    fn record_i64(&mut self, field: &tracing_core::Field, value: i64) {
        self.fields.push(proto::Field {
            name: Some(field.name().into()),
            value: Some(value.into()),
            metadata_id: Some(self.meta_id.clone()),
        });
    }

    fn record_u64(&mut self, field: &tracing_core::Field, value: u64) {
        self.fields.push(proto::Field {
            name: Some(field.name().into()),
            value: Some(value.into()),
            metadata_id: Some(self.meta_id.clone()),
        });
    }

    fn record_bool(&mut self, field: &tracing_core::Field, value: bool) {
        self.fields.push(proto::Field {
            name: Some(field.name().into()),
            value: Some(value.into()),
            metadata_id: Some(self.meta_id.clone()),
        });
    }

    fn record_str(&mut self, field: &tracing_core::Field, value: &str) {
        self.fields.push(proto::Field {
            name: Some(field.name().into()),
            value: Some(value.into()),
            metadata_id: Some(self.meta_id.clone()),
        });
    }
}

impl Visit for WakerVisitor {
    fn record_debug(&mut self, _: &field::Field, _: &dyn std::fmt::Debug) {
        // don't care (yet?)
    }

    fn record_u64(&mut self, field: &tracing_core::Field, value: u64) {
        if field.name() == "task.id" {
            self.id = Some(span::Id::from_u64(value));
        }
    }

    fn record_str(&mut self, field: &tracing_core::Field, value: &str) {
        if field.name() == "op" {
            self.op = Some(match value {
                "waker.wake" => WakeOp::Wake,
                "waker.wake_by_ref" => WakeOp::WakeByRef,
                "waker.clone" => WakeOp::Clone,
                "waker.drop" => WakeOp::Drop,
                _ => return,
            });
        }
    }
}
