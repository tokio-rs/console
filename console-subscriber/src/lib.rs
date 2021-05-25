use console_api as proto;
use tokio::sync::mpsc;

use std::{
    net::{IpAddr, Ipv4Addr, SocketAddr},
    ptr,
    sync::{
        atomic::{AtomicPtr, Ordering::*},
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

mod aggregator;
use aggregator::Aggregator;
mod builder;
pub use builder::Builder;

pub struct TasksLayer<F = DefaultFields> {
    task_meta: AtomicPtr<Metadata<'static>>,
    blocking_meta: AtomicPtr<Metadata<'static>>,
    tx: mpsc::Sender<Event>,
    flush: Arc<aggregator::Flush>,
    format: F,
}

pub struct Server {
    subscribe: mpsc::Sender<Watch>,
    addr: SocketAddr,
    aggregator: Option<Aggregator>,
    client_buffer: usize,
}

struct Watch(mpsc::Sender<Result<proto::tasks::TaskUpdate, tonic::Status>>);

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

impl TasksLayer {
    pub fn new() -> (Self, Server) {
        Self::builder().build()
    }

    /// Returns a [`Builder`] for configuring a `TasksLayer`.
    pub fn builder() -> Builder {
        Builder::default()
    }

    fn build(builder: Builder) -> (Self, Server) {
        // The `cfg` value *appears* to be a constant to clippy, but it changes
        // depending on the build-time configuration...
        #![allow(clippy::assertions_on_constants)]
        assert!(
            cfg!(tokio_unstable),
            "task tracing requires Tokio to be built with RUSTFLAGS=\"--cfg tokio_unstable\"!"
        );

        let (tx, events) = mpsc::channel(builder.event_buffer_capacity);
        let (subscribe, rpcs) = mpsc::channel(256);

        let aggregator = Aggregator::new(events, rpcs, builder.publish_interval);
        let flush = aggregator.flush().clone();
        let server = Server {
            aggregator: Some(aggregator),
            addr: builder.server_addr,
            subscribe,
            client_buffer: builder.client_buffer_capacity,
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
    pub const DEFAULT_PUBLISH_INTERVAL: Duration = Duration::from_secs(1);

    /// By default, completed spans are retained for one hour.
    pub const DEFAULT_RETENTION: Duration = Duration::from_secs(60 * 60);
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
            self.flush.trigger();
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
