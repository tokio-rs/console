use console_api as proto;
use tokio::sync::{mpsc, Notify};

use futures::FutureExt;
use std::{
    collections::HashMap,
    mem,
    net::SocketAddr,
    ops::{Deref, DerefMut},
    ptr,
    sync::{
        atomic::{AtomicBool, AtomicPtr, Ordering::*},
        Arc,
    },
    time::{Duration, SystemTime},
};
use tracing_core::{
    span,
    subscriber::{self, Subscriber},
    Metadata,
};
use tracing_subscriber::{
    fmt::{
        format::{DefaultFields, FormatFields},
        FormattedFields,
    },
    layer::Context,
    registry::LookupSpan,
    Layer,
};

enum Event {
    Metadata(&'static Metadata<'static>),
    Spawn {
        id: span::Id,
        metadata: &'static Metadata<'static>,
        at: SystemTime,
        fields: String,
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
}

pub struct TasksLayer<F = DefaultFields> {
    task_meta: AtomicPtr<Metadata<'static>>,
    blocking_meta: AtomicPtr<Metadata<'static>>,
    tx: mpsc::Sender<Event>,
    flush: Arc<Flush>,
    format: F,
}

pub struct Server {
    subscribe: mpsc::Sender<Watch>,
    addr: SocketAddr,
    aggregator: Option<Aggregator>,
    client_buffer: usize,
}

struct Aggregator {
    events: mpsc::Receiver<Event>,
    rpcs: mpsc::Receiver<Watch>,
    flush_interval: Duration,
    flush_capacity: Arc<Flush>,
}

struct Watch(mpsc::Sender<Result<proto::tasks::TaskUpdate, tonic::Status>>);

#[derive(Debug)]
struct Flush {
    should_flush: Notify,
    triggered: AtomicBool,
}

#[derive(Default)]
struct Stats {
    polls: u64,
    current_polls: u64,
    created_at: Option<SystemTime>,
    first_poll: Option<SystemTime>,
    last_poll: Option<SystemTime>,
    busy_time: Duration,
    closed_at: Option<SystemTime>,
}

#[derive(Default)]
struct TaskData<T> {
    data: HashMap<span::Id, (T, bool)>,
}

struct Task {
    metadata: &'static Metadata<'static>,
    fields: String,
}

impl TasksLayer {
    pub fn new() -> (Self, Server) {
        // The `cfg` value *appears* to be a constant to clippy, but it changes
        // depending on the build-time configuration...
        #![allow(clippy::assertions_on_constants)]
        assert!(
            cfg!(tokio_unstable),
            "task tracing requires Tokio to be built with RUSTFLAGS=\"--cfg tokio_unstable\"!"
        );
        // TODO(eliza): builder
        let (tx, events) = mpsc::channel(Self::DEFAULT_EVENT_BUFFER_CAPACITY);
        let (subscribe, rpcs) = mpsc::channel(256);
        let flush = Arc::new(Flush {
            should_flush: Notify::new(),
            triggered: AtomicBool::new(false),
        });
        let aggregator = Aggregator {
            events,
            rpcs,
            flush_interval: Self::DEFAULT_FLUSH_INTERVAL,
            flush_capacity: flush.clone(),
        };
        let addr = SocketAddr::from(([127, 0, 0, 1], 6669));
        let server = Server {
            aggregator: Some(aggregator),
            addr,
            subscribe,
            client_buffer: Self::DEFAULT_CLIENT_BUFFER_CAPACITY,
        };
        let layer = Self {
            tx,
            flush,
            task_meta: AtomicPtr::new(ptr::null_mut()),
            blocking_meta: AtomicPtr::new(ptr::null_mut()),
            format: Default::default(),
        };
        (layer, server)
    }
}

impl<F> TasksLayer<F> {
    pub const DEFAULT_EVENT_BUFFER_CAPACITY: usize = 1024 * 10;
    pub const DEFAULT_CLIENT_BUFFER_CAPACITY: usize = 1024 * 4;
    pub const DEFAULT_FLUSH_INTERVAL: Duration = Duration::from_secs(1);

    // how much capacity should remain in the buffer before triggering a
    // flush on capacity?
    //
    // chosen by fair die roll, guaranteed to be random :)
    const FLUSH_AT_CAPACITY: usize = 100;

    #[inline(always)]
    fn is_spawn(&self, meta: &'static Metadata<'static>) -> bool {
        ptr::eq(self.task_meta.load(Relaxed), meta as *const _ as *mut _)
        // || ptr::eq(self.blocking_meta.load(Relaxed), meta as *const _ as *mut _)
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
            if self
                .flush
                .triggered
                .compare_exchange(false, true, AcqRel, Acquire)
                .is_ok()
            {
                self.flush.should_flush.notify_one();
                tracing::trace!("flush triggered");
            } else {
                // someone else already did it, that's fine...
                tracing::trace!("flush already triggered");
            }
        }
    }
}

impl<S, F> Layer<S> for TasksLayer<F>
where
    S: Subscriber + for<'a> LookupSpan<'a>,
    F: for<'writer> FormatFields<'writer> + 'static,
{
    fn register_callsite(&self, meta: &'static Metadata<'static>) -> subscriber::Interest {
        if meta.target() == "tokio::task" && meta.name() == "task" {
            if meta.fields().iter().any(|f| f.name() == "function") {
                let _ = self.blocking_meta.compare_exchange(
                    ptr::null_mut(),
                    meta as *const _ as *mut _,
                    AcqRel,
                    Acquire,
                );
            } else {
                let _ = self.task_meta.compare_exchange(
                    ptr::null_mut(),
                    meta as *const _ as *mut _,
                    AcqRel,
                    Acquire,
                );
            }
        }

        self.send(Event::Metadata(meta));

        subscriber::Interest::always()
    }

    fn new_span(&self, attrs: &span::Attributes<'_>, id: &span::Id, cx: Context<'_, S>) {
        let metadata = attrs.metadata();
        if self.is_spawn(metadata) {
            let at = SystemTime::now();
            let span = cx.span(id).expect("newly-created span should exist");
            let mut exts = span.extensions_mut();
            let fields = match exts.get_mut::<FormattedFields<F>>() {
                Some(fields) => fields.fields.clone(),
                None => {
                    let mut fields = String::new();
                    match self.format.format_fields(&mut fields, attrs) {
                        Ok(()) => exts.insert(FormattedFields::<F>::new(fields.clone())),
                        Err(_) => {
                            tracing::warn!(span.id = ?id, span.attrs = ?attrs, "error formatting fields for span")
                        }
                    }
                    fields
                }
            };
            self.send(Event::Spawn {
                id: id.clone(),
                at,
                metadata,
                fields,
            });
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
    pub fn with_addr(self, addr: impl Into<SocketAddr>) -> Self {
        Self {
            addr: addr.into(),
            ..self
        }
    }

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
        permit.send(Watch(tx));
        tracing::debug!("watch started");
        let stream = tokio_stream::wrappers::ReceiverStream::new(rx);
        Ok(tonic::Response::new(stream))
    }
}

impl Aggregator {
    async fn run(mut self) {
        let mut flush = tokio::time::interval(self.flush_interval);
        let mut watches = Vec::new();
        let mut metadata = Vec::new();
        let mut new_metadata = Vec::new();
        let mut tasks = TaskData {
            data: HashMap::<span::Id, (Task, bool)>::new(),
        };
        let mut stats = TaskData::<Stats>::default();
        let mut completed_spans = Vec::new();
        loop {
            let should_send = tokio::select! {
                // if the flush interval elapses, flush data to the client
                _ = flush.tick() => {
                    true
                }
                // triggered when the event buffer is approaching capacity
                _ = self.flush_capacity.should_flush.notified() => {
                    self.flush_capacity.triggered.store(false, Release);
                    tracing::debug!("approaching capacity; draining buffer");
                    false
                }
                subscription = self.rpcs.recv() => {
                    if let Some(subscription) = subscription {
                        tracing::debug!("new subscription");
                        let new_tasks = tasks.all().map(|(id, task)| {
                            task.to_proto(id.clone())
                        }).collect();
                        let now = SystemTime::now();
                        let stats_update = stats.all().map(|(id, stats)| {
                            (id.into_u64(), stats.to_proto(now))
                        }).collect();
                        // Send the initial state --- if this fails, the subscription is
                        // already dead.
                        if subscription.update(&proto::tasks::TaskUpdate {
                            new_metadata: Some(proto::RegisterMetadata {
                                metadata: metadata.clone(),
                            }),
                            new_tasks,
                            stats_update,
                            ..Default::default()
                        }) {
                            watches.push(subscription)
                        }
                    } else {
                        tracing::debug!("rpc channel closed, terminating");
                        return;
                    }

                    false
                }

            };

            // drain and aggregate buffered events.
            //
            // Note: we *don't* want to actually await the call to `recv` --- we
            // don't want the aggregator task to be woken on every event,
            // because it will then be woken when its own `poll` calls are
            // exited. that would result in a busy-loop. instead, we only want
            // to be woken when the flush interval has elapsed, or when the
            // channel is almost full.
            while let Some(event) = self.events.recv().now_or_never() {
                let event = match event {
                    Some(event) => event,
                    None => return,
                };

                // do state update
                match event {
                    Event::Metadata(meta) => {
                        metadata.push(meta.into());
                        new_metadata.push(meta.into());
                    }
                    Event::Spawn {
                        id,
                        metadata,
                        at,
                        fields,
                    } => {
                        tasks.insert(
                            id.clone(),
                            Task {
                                metadata,
                                fields,
                                // TODO: parents
                            },
                        );
                        stats.insert(
                            id,
                            Stats {
                                polls: 0,
                                created_at: Some(at),
                                ..Default::default()
                            },
                        );
                    }
                    Event::Enter { id, at } => {
                        let mut stats = stats.update_or_default(id);
                        if stats.current_polls == 0 {
                            stats.last_poll = Some(at);
                            if stats.first_poll == None {
                                stats.first_poll = Some(at);
                            }
                            stats.polls += 1;
                        }
                        stats.current_polls += 1;
                    }

                    Event::Exit { id, at } => {
                        let mut stats = stats.update_or_default(id);
                        stats.current_polls -= 1;
                        if stats.current_polls == 0 {
                            if let Some(last_poll) = stats.last_poll {
                                stats.busy_time += at.duration_since(last_poll).unwrap();
                            }
                        }
                    }

                    Event::Close { id, at } => {
                        stats.update_or_default(id.clone()).closed_at = Some(at);
                        completed_spans.push(id.into());
                    }
                }
            }

            // flush data to clients
            if should_send {
                let new_metadata = if !new_metadata.is_empty() {
                    Some(proto::RegisterMetadata {
                        metadata: mem::replace(&mut new_metadata, Vec::new()),
                    })
                } else {
                    None
                };
                let new_tasks = tasks
                    .since_last_update()
                    .map(|(id, task)| task.to_proto(id.clone()))
                    .collect();
                let now = SystemTime::now();
                let stats_update = stats
                    .since_last_update()
                    .map(|(id, stats)| (id.into_u64(), stats.to_proto(now)))
                    .collect();
                let update = proto::tasks::TaskUpdate {
                    new_metadata,
                    new_tasks,
                    stats_update,
                    completed: mem::replace(&mut completed_spans, Vec::new()),
                    now: Some(now.into()),
                };
                watches.retain(|watch: &Watch| watch.update(&update));
            }
        }
    }
}

impl<T> TaskData<T> {
    fn update_or_default(&mut self, id: span::Id) -> Updating<'_, T>
    where
        T: Default,
    {
        Updating(self.data.entry(id).or_default())
    }

    // fn update(&mut self, id: &span::Id) -> Option<Updating<'_, T>> {
    //     Some(Updating(self.data.get_mut(id)?))
    // }

    fn insert(&mut self, id: span::Id, data: T) {
        self.data.insert(id, (data, true));
    }

    fn since_last_update(&mut self) -> impl Iterator<Item = (&span::Id, &mut T)> {
        self.data.iter_mut().filter_map(|(id, (data, dirty))| {
            if *dirty {
                *dirty = false;
                Some((id, data))
            } else {
                None
            }
        })
    }

    fn all(&self) -> impl Iterator<Item = (&span::Id, &T)> {
        self.data.iter().map(|(id, (data, _))| (id, data))
    }
}

struct Updating<'a, T>(&'a mut (T, bool));

impl<'a, T> Deref for Updating<'a, T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        &self.0 .0
    }
}

impl<'a, T> DerefMut for Updating<'a, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0 .0
    }
}

impl<'a, T> Drop for Updating<'a, T> {
    fn drop(&mut self) {
        self.0 .1 = true;
    }
}

impl Watch {
    fn update(&self, update: &proto::tasks::TaskUpdate) -> bool {
        if let Ok(reserve) = self.0.try_reserve() {
            reserve.send(Ok(update.clone()));
            true
        } else {
            false
        }
    }
}

impl Stats {
    fn total_time(&self, now: SystemTime) -> Option<Duration> {
        let now = self.closed_at.unwrap_or(now);
        self.created_at
            .and_then(|then| now.duration_since(then).ok())
    }

    fn to_proto(&self, now: SystemTime) -> proto::tasks::Stats {
        proto::tasks::Stats {
            polls: self.polls,
            created_at: self.created_at.map(Into::into),
            first_poll: self.created_at.map(Into::into),
            last_poll: self.created_at.map(Into::into),
            busy_time: Some(self.busy_time.into()),
            total_time: self.total_time(now).map(Into::into),
        }
    }
}

impl Task {
    fn to_proto(&self, id: span::Id) -> proto::tasks::Task {
        proto::tasks::Task {
            id: Some(id.into()),
            // TODO: more kinds of tasks...
            kind: proto::tasks::task::Kind::Spawn as i32,
            string_fields: self.fields.clone(),
            metadata: Some(self.metadata.into()),
            parents: Vec::new(), // TODO: implement parents nicely
        }
    }
}
