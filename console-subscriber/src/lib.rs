use console_api as proto;
use proto::{resources::resource, SpanId};

use tokio::sync::{mpsc, oneshot};

use std::{
    cell::RefCell,
    net::{IpAddr, Ipv4Addr, SocketAddr},
    sync::Arc,
    time::{Duration, SystemTime},
};
use thread_local::ThreadLocal;
use tracing_core::{
    span,
    subscriber::{self, Subscriber},
    Metadata,
};
use tracing_subscriber::{layer::Context, registry::LookupSpan, Layer};

mod aggregator;
mod builder;
mod callsites;
mod init;
mod stack;
mod visitors;

use aggregator::Aggregator;
pub use builder::Builder;
use callsites::Callsites;
use stack::SpanStack;
use visitors::{
    AsyncOpVisitor, FieldVisitor, ResourceOpData, ResourceOpVisitor, ResourceVisitor, WakerVisitor,
};

pub use init::{build, init};

pub struct TasksLayer {
    current_spans: ThreadLocal<RefCell<SpanStack>>,
    tx: mpsc::Sender<Event>,
    flush: Arc<aggregator::Flush>,
    spawn_callsites: Callsites,
    waker_callsites: Callsites,
    resource_callsites: Callsites,
    async_op_callsites: Callsites,
    resource_op_callsites: Callsites,
}

pub struct Server {
    subscribe: mpsc::Sender<WatchKind>,
    addr: SocketAddr,
    aggregator: Option<Aggregator>,
    client_buffer: usize,
}

struct Watch<T>(mpsc::Sender<Result<T, tonic::Status>>);

enum WatchKind {
    Instrument(Watch<proto::instrument::InstrumentUpdate>),
    TaskDetail(WatchRequest<proto::tasks::TaskDetails>),
}

struct WatchRequest<T> {
    id: SpanId,
    stream_sender: oneshot::Sender<mpsc::Receiver<Result<T, tonic::Status>>>,
    buffer: usize,
}

#[derive(Debug)]
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
    Resource {
        id: span::Id,
        metadata: &'static Metadata<'static>,
        at: SystemTime,
        concrete_type: String,
        kind: resource::Kind,
    },
    ResourceOp {
        metadata: &'static Metadata<'static>,
        at: SystemTime,
        resource_id: span::Id,
        op_name: String,
        op_type: OpType,
    },
    AsyncResourceOp {
        id: span::Id,
        metadata: &'static Metadata<'static>,
        at: SystemTime,
        source: String,
    },
}

#[derive(Debug, Clone)]
enum Readiness {
    Pending,
    Ready,
}

#[derive(Clone, Debug)]
enum OpType {
    Poll {
        async_op_id: span::Id,
        task_id: span::Id,
        readiness: Readiness,
    },
    StateUpdate(Vec<AttributeUpdate>),
}

#[derive(Debug, Clone)]
struct AttributeUpdate {
    name: String,
    val: AttributeUpdateValue,
}

#[derive(Debug, Clone)]
enum AttributeUpdateValue {
    Text(String),
    Numeric {
        val: u64,
        op: AttributeUpdateOp,
        unit: String,
    },
}

#[derive(Debug, Clone)]
enum AttributeUpdateOp {
    Add,
    Ovr,
    Sub,
}

#[derive(Debug)]
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
            resource_callsites: Callsites::default(),
            async_op_callsites: Callsites::default(),
            resource_op_callsites: Callsites::default(),
            current_spans: ThreadLocal::new(),
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

    fn is_resource(&self, meta: &'static Metadata<'static>) -> bool {
        self.resource_callsites.contains(meta)
    }

    fn is_async_op(&self, meta: &'static Metadata<'static>) -> bool {
        self.async_op_callsites.contains(meta)
    }

    fn is_resource_op(&self, meta: &'static Metadata<'static>) -> bool {
        self.resource_op_callsites.contains(meta)
    }

    fn is_id_spawned<S>(&self, id: &span::Id, cx: &Context<'_, S>) -> bool
    where
        S: Subscriber + for<'a> LookupSpan<'a>,
    {
        cx.span(id)
            .map(|span| self.is_spawn(span.metadata()))
            .unwrap_or(false)
    }

    fn is_id_resource<S>(&self, id: &span::Id, cx: &Context<'_, S>) -> bool
    where
        S: Subscriber + for<'a> LookupSpan<'a>,
    {
        cx.span(id)
            .map(|span| self.is_resource(span.metadata()))
            .unwrap_or(false)
    }

    fn is_id_async_op<S>(&self, id: &span::Id, cx: &Context<'_, S>) -> bool
    where
        S: Subscriber + for<'a> LookupSpan<'a>,
    {
        cx.span(id)
            .map(|span| self.is_async_op(span.metadata()))
            .unwrap_or(false)
    }

    fn is_id_resource_op<S>(&self, id: &span::Id, cx: &Context<'_, S>) -> bool
    where
        S: Subscriber + for<'a> LookupSpan<'a>,
    {
        cx.span(id)
            .map(|span| self.is_resource_op(span.metadata()))
            .unwrap_or(false)
    }

    fn is_id_tracked<S>(&self, id: &span::Id, cx: &Context<'_, S>) -> bool
    where
        S: Subscriber + for<'a> LookupSpan<'a>,
    {
        self.is_id_async_op(id, cx)
            || self.is_id_resource(id, cx)
            || self.is_id_spawned(id, cx)
            || self.is_id_resource_op(id, cx)
    }

    fn first_entered<P>(&self, stack: &SpanStack, p: P) -> Option<span::Id>
    where
        P: Fn(&span::Id) -> bool,
    {
        return stack
            .stack()
            .iter()
            .rev()
            .find(|id| p(id.id()))
            .map(|id| id.id())
            .cloned();
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
        } else if meta.name() == "runtime.resource" {
            self.resource_callsites.insert(meta);
        } else if meta.name() == "runtime.async_op" {
            self.async_op_callsites.insert(meta);
        } else if meta.target() == "tokio::resource::op" {
            self.resource_op_callsites.insert(meta);
        }
        self.send(Event::Metadata(meta));
        subscriber::Interest::always()
    }

    fn new_span(&self, attrs: &span::Attributes<'_>, id: &span::Id, _: Context<'_, S>) {
        let metadata = attrs.metadata();
        if self.is_spawn(metadata) {
            let at = SystemTime::now();
            let mut field_visitor = FieldVisitor::new(metadata.into());
            attrs.record(&mut field_visitor);
            self.send(Event::Spawn {
                id: id.clone(),
                at,
                metadata,
                fields: field_visitor.result(),
            });
        } else if self.is_resource(metadata) {
            let mut resource_visitor = ResourceVisitor::default();
            attrs.record(&mut resource_visitor);
            match resource_visitor.result() {
                Some((concrete_type, kind)) => {
                    let at = SystemTime::now();
                    self.send(Event::Resource {
                        id: id.clone(),
                        metadata,
                        at,
                        concrete_type,
                        kind,
                    });
                }
                _ => tracing::warn!("unknown resource span format: {:?}", attrs),
            }
        } else if self.is_async_op(metadata) {
            let mut async_op_visitor = AsyncOpVisitor::default();
            attrs.record(&mut async_op_visitor);
            if let Some(source) = async_op_visitor.result() {
                let at = SystemTime::now();
                self.send(Event::AsyncResourceOp {
                    id: id.clone(),
                    at,
                    metadata,
                    source,
                });
            } else {
                tracing::warn!("async op span needs to have a source field: {:?}", attrs);
            }
        }
    }

    fn on_event(&self, event: &tracing::Event<'_>, ctx: Context<'_, S>) {
        let metadata = event.metadata();
        if self.waker_callsites.contains(event.metadata()) {
            let at = SystemTime::now();
            let mut visitor = WakerVisitor::default();
            event.record(&mut visitor);
            match visitor.result() {
                Some((id, op)) => self.send(Event::Waker { id, op, at }),
                None => tracing::warn!("unknown waker event: {:?}", event),
            }
        } else if self.resource_op_callsites.contains(event.metadata()) {
            match ctx.current_span().id() {
                Some(resource_id) if self.is_id_resource(resource_id, &ctx) => {
                    let mut resource_op_visitor = ResourceOpVisitor::default();
                    event.record(&mut resource_op_visitor);
                    let res = resource_op_visitor.result();
                    let op_data = match res {
                        Some(ResourceOpData::Poll { op_name, readiness }) => self
                            .current_spans
                            .get()
                            .and_then(|stack| {
                                let stack = stack.borrow();
                                let task_id =
                                    self.first_entered(&stack, |id| self.is_id_spawned(id, &ctx));

                                let async_op_id =
                                    self.first_entered(&stack, |id| self.is_id_async_op(id, &ctx));

                                task_id.zip(async_op_id)
                            })
                            .map(|(task_id, async_op_id)| {
                                let op_type = OpType::Poll {
                                    async_op_id,
                                    task_id,
                                    readiness,
                                };

                                (op_name, op_type)
                            }),
                        Some(ResourceOpData::StateUpdate { op_name, attrs }) => {
                            Some((op_name, OpType::StateUpdate(attrs)))
                        }
                        None => None,
                    };

                    if let Some((op_name, op_type)) = op_data {
                        let at = SystemTime::now();
                        self.send(Event::ResourceOp {
                            metadata,
                            at,
                            resource_id: resource_id.clone(),
                            op_name,
                            op_type,
                        });
                    } else {
                        tracing::warn!("resource op event has invalid format: {:?}", event);
                    }
                }
                _ => tracing::warn!(
                    "resource op event should be emitted in the context of a resource span: {:?}",
                    event
                ),
            }
        }
    }

    fn on_enter(&self, id: &span::Id, cx: Context<'_, S>) {
        if !self.is_id_tracked(id, &cx) {
            return;
        }

        self.current_spans
            .get_or_default()
            .borrow_mut()
            .push(id.clone());

        self.send(Event::Enter {
            at: SystemTime::now(),
            id: id.clone(),
        });
    }

    fn on_exit(&self, id: &span::Id, cx: Context<'_, S>) {
        if !self.is_id_tracked(id, &cx) {
            return;
        }

        if let Some(spans) = self.current_spans.get() {
            spans.borrow_mut().pop(id);
        }

        self.send(Event::Exit {
            at: SystemTime::now(),
            id: id.clone(),
        });
    }

    fn on_close(&self, id: span::Id, cx: Context<'_, S>) {
        if !self.is_id_tracked(&id, &cx) {
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
            .add_service(proto::instrument::instrument_server::InstrumentServer::new(
                self,
            ))
            .serve(addr)
            .await;
        aggregate.abort();
        res.map_err(Into::into)
    }
}

#[tonic::async_trait]
impl proto::instrument::instrument_server::Instrument for Server {
    type WatchUpdatesStream = tokio_stream::wrappers::ReceiverStream<
        Result<proto::instrument::InstrumentUpdate, tonic::Status>,
    >;
    type WatchTaskDetailsStream =
        tokio_stream::wrappers::ReceiverStream<Result<proto::tasks::TaskDetails, tonic::Status>>;
    async fn watch_updates(
        &self,
        req: tonic::Request<proto::instrument::InstrumentRequest>,
    ) -> Result<tonic::Response<Self::WatchUpdatesStream>, tonic::Status> {
        match req.remote_addr() {
            Some(addr) => tracing::debug!(client.addr = %addr, "starting a new watch"),
            None => tracing::debug!(client.addr = %"<unknown>", "starting a new watch"),
        }
        let permit = self.subscribe.reserve().await.map_err(|_| {
            tonic::Status::internal("cannot start new watch, aggregation task is not running")
        })?;
        let (tx, rx) = mpsc::channel(self.client_buffer);
        permit.send(WatchKind::Instrument(Watch(tx)));
        tracing::debug!("watch started");
        let stream = tokio_stream::wrappers::ReceiverStream::new(rx);
        Ok(tonic::Response::new(stream))
    }

    async fn watch_task_details(
        &self,
        req: tonic::Request<proto::instrument::TaskDetailsRequest>,
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
            id: task_id.clone(),
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
