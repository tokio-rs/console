#![doc = include_str!("../README.md")]
use console_api as proto;
use proto::resources::resource;
use serde::Serialize;
use std::{
    cell::RefCell,
    fmt,
    net::{IpAddr, Ipv4Addr, SocketAddr},
    sync::Arc,
    time::{Duration, SystemTime},
};
use thread_local::ThreadLocal;
use tokio::sync::{mpsc, oneshot};
use tracing_core::{
    dispatcher::{self, Dispatch},
    span,
    subscriber::{self, NoSubscriber, Subscriber},
    Metadata,
};
use tracing_subscriber::{layer::Context, registry::LookupSpan, Layer};

mod aggregator;
mod builder;
mod callsites;
mod record;
mod stack;
pub(crate) mod sync;
mod visitors;

use aggregator::Aggregator;
pub use builder::Builder;
use callsites::Callsites;
use stack::SpanStack;
use visitors::{AsyncOpVisitor, ResourceVisitor, ResourceVisitorResult, TaskVisitor, WakerVisitor};

pub use builder::{init, spawn};

use crate::aggregator::Id;
use crate::visitors::{PollOpVisitor, StateUpdateVisitor};

/// A [`ConsoleLayer`] is a [`tracing_subscriber::Layer`] that catches the tracing
/// events of the runtime (e.g. call-sites), and translating them into more
/// abstract events (e.g. task spawns or waker operations) that are meaningful
/// to modeling the async runtime, and then transmitting those abstact events
/// to an associated [`Server`] for aggregation.
pub struct ConsoleLayer {
    current_spans: ThreadLocal<RefCell<SpanStack>>,
    tx: mpsc::Sender<Event>,
    flush: Arc<aggregator::Flush>,
    /// When the channel capacity goes under this number, a flush in the aggregator
    /// will be triggered.
    flush_under_capacity: usize,

    /// Set of callsites for spans representing spawned tasks.
    ///
    /// For task spans, each runtime these will have like, 1-5 callsites in it, max, so
    /// 8 should be plenty. If several runtimes are in use, we may have to spill
    /// over into the backup hashmap, but it's unlikely.
    spawn_callsites: Callsites<8>,

    /// Set of callsites for events representing waker operations.
    ///
    /// 16 is probably a reasonable number of waker ops; it's a bit generous if
    /// there's only one async runtime library in use, but if there are multiple,
    /// they might all have their own sets of waker ops.
    waker_callsites: Callsites<16>,

    /// Set of callsites for spans representing resources
    ///
    /// TODO: Take some time to determine more reasonable numbers
    resource_callsites: Callsites<32>,

    /// Set of callsites for spans representing async operations on resources
    ///
    /// TODO: Take some time to determine more reasonable numbers
    async_op_callsites: Callsites<32>,

    /// Set of callsites for spans representing async op poll operations
    ///
    /// TODO: Take some time to determine more reasonable numbers
    async_op_poll_callsites: Callsites<32>,

    /// Set of callsites for events representing poll operation invocations on resources
    ///
    /// TODO: Take some time to determine more reasonable numbers
    poll_op_callsites: Callsites<32>,

    /// Set of callsites for events representing state attribute state updates on resources
    ///
    /// TODO: Take some time to determine more reasonable numbers
    resource_state_update_callsites: Callsites<32>,

    /// Set of callsites for events representing state attribute state updates on async resource ops
    ///
    /// TODO: Take some time to determine more reasonable numbers
    async_op_state_update_callsites: Callsites<32>,

    /// Used for unsetting the default dispatcher inside of span callbacks.
    no_dispatch: Dispatch,
}

/// A [`Server`] is in charge of aggregating the abstract runtime events into a
/// model of the runtime's history (such as a list of the currently active
/// tasks, or statistics summarizing polling times). A [`Server`] also
/// interprets commands from a client application, such a request to focus in on
/// a specific task, and translates that into a stream of details specific to
/// that task.
pub struct Server {
    subscribe: mpsc::Sender<Command>,
    addr: SocketAddr,
    aggregator: Option<Aggregator>,
    client_buffer: usize,
}

struct Watch<T>(mpsc::Sender<Result<T, tonic::Status>>);

enum Command {
    Instrument(Watch<proto::instrument::Update>),
    WatchTaskDetail(WatchRequest<proto::tasks::TaskDetails>),
    Pause,
    Resume,
}

struct WatchRequest<T> {
    id: Id,
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
        location: Option<proto::Location>,
    },
    Enter {
        id: span::Id,
        parent_id: Option<span::Id>,
        at: SystemTime,
    },
    Exit {
        id: span::Id,
        parent_id: Option<span::Id>,
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
        parent_id: Option<span::Id>,
        metadata: &'static Metadata<'static>,
        at: SystemTime,
        concrete_type: String,
        kind: resource::Kind,
        location: Option<proto::Location>,
        is_internal: bool,
        inherit_child_attrs: bool,
    },
    PollOp {
        metadata: &'static Metadata<'static>,
        resource_id: span::Id,
        op_name: String,
        async_op_id: span::Id,
        task_id: span::Id,
        is_ready: bool,
    },
    StateUpdate {
        update_id: span::Id,
        update_type: UpdateType,
        update: AttributeUpdate,
    },
    AsyncResourceOp {
        id: span::Id,
        parent_id: Option<span::Id>,
        resource_id: span::Id,
        metadata: &'static Metadata<'static>,
        at: SystemTime,
        source: String,
        inherit_child_attrs: bool,
    },
}

#[derive(Debug, Clone)]
enum UpdateType {
    Resource,
    AsyncOp,
}

#[derive(Debug, Clone)]
struct AttributeUpdate {
    field: proto::Field,
    op: Option<AttributeUpdateOp>,
    unit: Option<String>,
}

#[derive(Debug, Clone)]
enum AttributeUpdateOp {
    Add,
    Override,
    Sub,
}

#[derive(Clone, Debug, Copy, Serialize)]
enum WakeOp {
    Wake { self_wake: bool },
    WakeByRef { self_wake: bool },
    Clone,
    Drop,
}

impl ConsoleLayer {
    /// Returns a `ConsoleLayer` built with the default settings.
    ///
    /// Note: these defaults do *not* include values provided via the
    /// environment variables specified in [`Builder::with_default_env`].
    ///
    /// See also [`Builder::build`].
    pub fn new() -> (Self, Server) {
        Self::builder().build()
    }

    /// Returns a [`Builder`] for configuring a `ConsoleLayer`.
    ///
    /// Note that the returned builder does *not* include values provided via
    /// the environment variables specified in [`Builder::with_default_env`].
    /// To extract those, you can call that method on the returned builder.
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

        // Conservatively, start to trigger a flush when half the channel is full.
        // This tries to reduce the chance of losing events to a full channel.
        let flush_under_capacity = config.event_buffer_capacity / 2;

        let server = Server {
            aggregator: Some(aggregator),
            addr: config.server_addr,
            subscribe,
            client_buffer: config.client_buffer_capacity,
        };
        let layer = Self {
            current_spans: ThreadLocal::new(),
            tx,
            flush,
            flush_under_capacity,
            spawn_callsites: Callsites::default(),
            waker_callsites: Callsites::default(),
            resource_callsites: Callsites::default(),
            async_op_callsites: Callsites::default(),
            async_op_poll_callsites: Callsites::default(),
            poll_op_callsites: Callsites::default(),
            resource_state_update_callsites: Callsites::default(),
            async_op_state_update_callsites: Callsites::default(),
            no_dispatch: Dispatch::new(NoSubscriber::default()),
        };
        (layer, server)
    }
}

impl ConsoleLayer {
    /// Default maximum capacity for channel of events from [`ConsoleLayer`] to [`Server`].
    ///
    /// When this capacity is exhausted, additional events will be dropped.
    ///
    /// See also [`Builder::event_buffer_capacity`].
    pub const DEFAULT_EVENT_BUFFER_CAPACITY: usize = 1024 * 10;
    /// Default maximum capacity for channel of events from [`Server`] to each subscribed client.
    ///
    /// When this capacity is exhausted, the client may be disconnected.
    ///
    /// See also [`Builder::client_buffer_capacity`].
    pub const DEFAULT_CLIENT_BUFFER_CAPACITY: usize = 1024 * 4;

    /// Default frequency for publication of events to clients.
    ///
    /// Note that constructors like [`build`][`crate::build`] will take the
    /// value from the `TOKIO_CONSOLE_PUBLISH_INTERVAL` environment variable
    /// before falling back on this default.
    ///
    /// See also [`Builder::publish_interval`].
    pub const DEFAULT_PUBLISH_INTERVAL: Duration = Duration::from_secs(1);

    /// By default, completed spans are retained for one hour.
    ///
    /// Note that constructors like [`build`][`crate::build`] will take the
    /// value from the `TOKIO_CONSOLE_RETENTION` environment variable before
    /// falling back on this default.
    ///
    /// See also [`Builder::retention`].
    pub const DEFAULT_RETENTION: Duration = Duration::from_secs(60 * 60);

    fn is_spawn(&self, meta: &'static Metadata<'static>) -> bool {
        self.spawn_callsites.contains(meta)
    }

    fn is_resource(&self, meta: &'static Metadata<'static>) -> bool {
        self.resource_callsites.contains(meta)
    }

    fn is_async_op(&self, meta: &'static Metadata<'static>) -> bool {
        self.async_op_callsites.contains(meta)
    }

    fn is_async_op_poll(&self, meta: &'static Metadata<'static>) -> bool {
        self.async_op_poll_callsites.contains(meta)
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

    fn is_id_async_op_poll<S>(&self, id: &span::Id, cx: &Context<'_, S>) -> bool
    where
        S: Subscriber + for<'a> LookupSpan<'a>,
    {
        cx.span(id)
            .map(|span| self.is_async_op_poll(span.metadata()))
            .unwrap_or(false)
    }

    fn is_id_tracked<S>(&self, id: &span::Id, cx: &Context<'_, S>) -> bool
    where
        S: Subscriber + for<'a> LookupSpan<'a>,
    {
        self.is_id_async_op(id, cx)
            || self.is_id_resource(id, cx)
            || self.is_id_spawned(id, cx)
            || self.is_id_async_op_poll(id, cx)
    }

    fn first_entered<P>(&self, stack: &SpanStack, p: P) -> Option<span::Id>
    where
        P: Fn(&span::Id) -> bool,
    {
        stack
            .stack()
            .iter()
            .rev()
            .find(|id| p(id.id()))
            .map(|id| id.id())
            .cloned()
    }

    fn send(&self, event: Event) {
        use mpsc::error::TrySendError;

        match self.tx.try_reserve() {
            Ok(permit) => permit.send(event),
            Err(TrySendError::Closed(_)) => {
                // we should warn here eventually, but nop for now because we
                // can't trigger tracing events...
            }
            Err(TrySendError::Full(_)) => {
                // this shouldn't happen, since we trigger a flush when
                // approaching the high water line...but if the executor wait
                // time is very high, maybe the aggregator task hasn't been
                // polled yet. so... eek?!
            }
        }

        let capacity = self.tx.capacity();
        if capacity <= self.flush_under_capacity {
            self.flush.trigger();
        }
    }
}

impl<S> Layer<S> for ConsoleLayer
where
    S: Subscriber + for<'a> LookupSpan<'a>,
{
    fn register_callsite(&self, meta: &'static Metadata<'static>) -> subscriber::Interest {
        match (meta.name(), meta.target()) {
            ("runtime.spawn", _) | ("task", "tokio::task") => self.spawn_callsites.insert(meta),
            (_, "runtime::waker") | (_, "tokio::task::waker") => self.waker_callsites.insert(meta),
            (ResourceVisitor::RES_SPAN_NAME, _) => self.resource_callsites.insert(meta),
            (AsyncOpVisitor::ASYNC_OP_SPAN_NAME, _) => self.async_op_callsites.insert(meta),
            ("runtime.resource.async_op.poll", _) => self.async_op_poll_callsites.insert(meta),
            (_, PollOpVisitor::POLL_OP_EVENT_TARGET) => self.poll_op_callsites.insert(meta),
            (_, StateUpdateVisitor::RE_STATE_UPDATE_EVENT_TARGET) => {
                self.resource_state_update_callsites.insert(meta)
            }
            (_, StateUpdateVisitor::AO_STATE_UPDATE_EVENT_TARGET) => {
                self.async_op_state_update_callsites.insert(meta)
            }
            (_, _) => {}
        }

        self.send(Event::Metadata(meta));
        subscriber::Interest::always()
    }

    fn on_new_span(&self, attrs: &span::Attributes<'_>, id: &span::Id, ctx: Context<'_, S>) {
        let metadata = attrs.metadata();
        if self.is_spawn(metadata) {
            let at = SystemTime::now();
            let mut task_visitor = TaskVisitor::new(metadata.into());
            attrs.record(&mut task_visitor);
            let (fields, location) = task_visitor.result();
            self.send(Event::Spawn {
                id: id.clone(),
                at,
                metadata,
                fields,
                location,
            });
            return;
        }

        if self.is_resource(metadata) {
            let mut resource_visitor = ResourceVisitor::default();
            attrs.record(&mut resource_visitor);
            if let Some(result) = resource_visitor.result() {
                let ResourceVisitorResult {
                    concrete_type,
                    kind,
                    location,
                    is_internal,
                    inherit_child_attrs,
                } = result;
                let at = SystemTime::now();
                let parent_id = self.current_spans.get().and_then(|stack| {
                    self.first_entered(&stack.borrow(), |id| self.is_id_resource(id, &ctx))
                });
                self.send(Event::Resource {
                    id: id.clone(),
                    parent_id,
                    metadata,
                    at,
                    concrete_type,
                    kind,
                    location,
                    is_internal,
                    inherit_child_attrs,
                });
            } // else unknown resource span format
            return;
        }

        if self.is_async_op(metadata) {
            let mut async_op_visitor = AsyncOpVisitor::default();
            attrs.record(&mut async_op_visitor);
            if let Some((source, inherit_child_attrs)) = async_op_visitor.result() {
                let at = SystemTime::now();
                let resource_id = self.current_spans.get().and_then(|stack| {
                    self.first_entered(&stack.borrow(), |id| self.is_id_resource(id, &ctx))
                });

                let parent_id = self.current_spans.get().and_then(|stack| {
                    self.first_entered(&stack.borrow(), |id| self.is_id_async_op(id, &ctx))
                });

                if let Some(resource_id) = resource_id {
                    self.send(Event::AsyncResourceOp {
                        id: id.clone(),
                        parent_id,
                        resource_id,
                        at,
                        metadata,
                        source,
                        inherit_child_attrs,
                    });
                }
            }
            // else async op span needs to have a source field
        }
    }

    fn on_event(&self, event: &tracing::Event<'_>, ctx: Context<'_, S>) {
        let metadata = event.metadata();
        if self.waker_callsites.contains(metadata) {
            let at = SystemTime::now();
            let mut visitor = WakerVisitor::default();
            event.record(&mut visitor);
            if let Some((id, mut op)) = visitor.result() {
                if op.is_wake() {
                    // Are we currently inside the task's span? If so, the task
                    // has woken itself.
                    let self_wake = self
                        .current_spans
                        .get()
                        .map(|spans| spans.borrow().iter().any(|span| span == &id))
                        .unwrap_or(false);
                    op = op.self_wake(self_wake);
                }
                self.send(Event::Waker { id, op, at });
            }
            // else unknown waker event... what to do? can't trace it from here...
            return;
        }

        if self.poll_op_callsites.contains(metadata) {
            let resource_id = self.current_spans.get().and_then(|stack| {
                self.first_entered(&stack.borrow(), |id| self.is_id_resource(id, &ctx))
            });
            // poll op event should have a resource span parent
            if let Some(resource_id) = resource_id {
                let mut poll_op_visitor = PollOpVisitor::default();
                event.record(&mut poll_op_visitor);
                if let Some((op_name, is_ready)) = poll_op_visitor.result() {
                    let task_and_async_op_ids = self.current_spans.get().and_then(|stack| {
                        let stack = stack.borrow();
                        let task_id =
                            self.first_entered(&stack, |id| self.is_id_spawned(id, &ctx))?;
                        let async_op_id =
                            self.first_entered(&stack, |id| self.is_id_async_op(id, &ctx))?;
                        Some((task_id, async_op_id))
                    });

                    // poll op event should be emitted in the context of an async op and task spans
                    if let Some((task_id, async_op_id)) = task_and_async_op_ids {
                        self.send(Event::PollOp {
                            metadata,
                            op_name,
                            resource_id,
                            async_op_id,
                            task_id,
                            is_ready,
                        });
                    }
                }
            }
            return;
        }

        if self.resource_state_update_callsites.contains(metadata) {
            // state update event should have a resource span parent
            let resource_id = self.current_spans.get().and_then(|stack| {
                self.first_entered(&stack.borrow(), |id| self.is_id_resource(id, &ctx))
            });

            if let Some(resource_id) = resource_id {
                let meta_id = event.metadata().into();
                let mut state_update_visitor = StateUpdateVisitor::new(meta_id);
                event.record(&mut state_update_visitor);
                if let Some(update) = state_update_visitor.result() {
                    self.send(Event::StateUpdate {
                        update_id: resource_id,
                        update_type: UpdateType::Resource,
                        update,
                    })
                }
            }
            return;
        }

        if self.async_op_state_update_callsites.contains(metadata) {
            let async_op_id = self.current_spans.get().and_then(|stack| {
                self.first_entered(&stack.borrow(), |id| self.is_id_async_op(id, &ctx))
            });
            if let Some(async_op_id) = async_op_id {
                let meta_id = event.metadata().into();
                let mut state_update_visitor = StateUpdateVisitor::new(meta_id);
                event.record(&mut state_update_visitor);
                if let Some(update) = state_update_visitor.result() {
                    self.send(Event::StateUpdate {
                        update_id: async_op_id,
                        update_type: UpdateType::AsyncOp,
                        update,
                    });
                }
            }
        }
    }

    fn on_enter(&self, id: &span::Id, cx: Context<'_, S>) {
        if !self.is_id_tracked(id, &cx) {
            return;
        }
        let _default = dispatcher::set_default(&self.no_dispatch);
        self.current_spans
            .get_or_default()
            .borrow_mut()
            .push(id.clone());

        let parent_id = cx.span(id).and_then(|s| s.parent().map(|p| p.id()));
        self.send(Event::Enter {
            at: SystemTime::now(),
            id: id.clone(),
            parent_id,
        });
    }

    fn on_exit(&self, id: &span::Id, cx: Context<'_, S>) {
        if !self.is_id_tracked(id, &cx) {
            return;
        }

        let _default = dispatcher::set_default(&self.no_dispatch);
        if let Some(spans) = self.current_spans.get() {
            spans.borrow_mut().pop(id);
        }

        let parent_id = cx.span(id).and_then(|s| s.parent().map(|p| p.id()));

        self.send(Event::Exit {
            id: id.clone(),
            parent_id,
            at: SystemTime::now(),
        });
    }

    fn on_close(&self, id: span::Id, cx: Context<'_, S>) {
        if !self.is_id_tracked(&id, &cx) {
            return;
        }

        let _default = dispatcher::set_default(&self.no_dispatch);
        self.send(Event::Close {
            at: SystemTime::now(),
            id,
        });
    }
}

impl fmt::Debug for ConsoleLayer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ConsoleLayer")
            // mpsc::Sender debug impl is not very useful
            .field("tx", &format_args!("<...>"))
            .field("tx.capacity", &self.tx.capacity())
            .field("flush", &self.flush)
            .field("spawn_callsites", &self.spawn_callsites)
            .field("waker_callsites", &self.waker_callsites)
            .finish()
    }
}

impl Server {
    // XXX(eliza): why is `SocketAddr::new` not `const`???
    /// A [`Server`] by default binds socket address 127.0.0.1 to service remote procedure calls.
    ///
    /// Note that constructors like [`build`][`crate::build`] will parse the
    /// socket address from the `TOKIO_CONSOLE_BIND` environment variable before
    /// falling back on constructing a socket address from this default.
    ///
    /// See also [`Builder::server_addr`].
    pub const DEFAULT_IP: IpAddr = IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1));

    /// A [`Server`] by default binds port 6669 to service remote procedure calls.
    ///
    /// Note that constructors like [`build`][`crate::build`] will parse the
    /// socket address from the `TOKIO_CONSOLE_BIND` environment variable before
    /// falling back on constructing a socket address from this default.
    ///
    /// See also [`Builder::server_addr`].
    pub const DEFAULT_PORT: u16 = 6669;

    /// Initiates service via the default gRPC transport service.
    pub async fn serve(self) -> Result<(), Box<dyn std::error::Error + Send + Sync + 'static>> {
        self.serve_with(tonic::transport::Server::default()).await
    }

    /// Initiates service via the given gRPC transport service `builder`.
    ///
    /// Both event aggregation and client command interpretation services are initiated here.
    pub async fn serve_with(
        mut self,
        mut builder: tonic::transport::Server,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync + 'static>> {
        let aggregate = self
            .aggregator
            .take()
            .expect("cannot start server multiple times");
        let aggregate = spawn_named(aggregate.run(), "console::aggregate");
        let addr = self.addr;
        let serve = builder
            .add_service(proto::instrument::instrument_server::InstrumentServer::new(
                self,
            ))
            .serve(addr);
        let res = spawn_named(serve, "console::serve").await;
        aggregate.abort();
        res?.map_err(Into::into)
    }
}

#[tonic::async_trait]
impl proto::instrument::instrument_server::Instrument for Server {
    type WatchUpdatesStream =
        tokio_stream::wrappers::ReceiverStream<Result<proto::instrument::Update, tonic::Status>>;
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
        permit.send(Command::Instrument(Watch(tx)));
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
        permit.send(Command::WatchTaskDetail(WatchRequest {
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

    async fn pause(
        &self,
        _req: tonic::Request<proto::instrument::PauseRequest>,
    ) -> Result<tonic::Response<proto::instrument::PauseResponse>, tonic::Status> {
        self.subscribe.send(Command::Pause).await.map_err(|_| {
            tonic::Status::internal("cannot pause, aggregation task is not running")
        })?;
        Ok(tonic::Response::new(proto::instrument::PauseResponse {}))
    }

    async fn resume(
        &self,
        _req: tonic::Request<proto::instrument::ResumeRequest>,
    ) -> Result<tonic::Response<proto::instrument::ResumeResponse>, tonic::Status> {
        self.subscribe.send(Command::Resume).await.map_err(|_| {
            tonic::Status::internal("cannot resume, aggregation task is not running")
        })?;
        Ok(tonic::Response::new(proto::instrument::ResumeResponse {}))
    }
}

impl WakeOp {
    /// Returns `true` if `self` is a `Wake` or `WakeByRef` event.
    fn is_wake(self) -> bool {
        matches!(self, Self::Wake { .. } | Self::WakeByRef { .. })
    }

    fn self_wake(self, self_wake: bool) -> Self {
        match self {
            Self::Wake { .. } => Self::Wake { self_wake },
            Self::WakeByRef { .. } => Self::WakeByRef { self_wake },
            x => x,
        }
    }
}

#[track_caller]
pub(crate) fn spawn_named<T>(
    task: impl std::future::Future<Output = T> + Send + 'static,
    _name: &str,
) -> tokio::task::JoinHandle<T>
where
    T: Send + 'static,
{
    #[cfg(tokio_unstable)]
    return tokio::task::Builder::new().name(_name).spawn(task);

    #[cfg(not(tokio_unstable))]
    tokio::spawn(task)
}
