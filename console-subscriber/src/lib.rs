use console_api as proto;
use tokio::sync::mpsc;

use std::{
    net::SocketAddr,
    ptr,
    sync::{
        atomic::{AtomicPtr, Ordering::*},
        Arc,
    },
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
use aggregator::Aggregator;

pub struct TasksLayer {
    task_meta: AtomicPtr<Metadata<'static>>,
    blocking_meta: AtomicPtr<Metadata<'static>>,
    tx: mpsc::Sender<Event>,
    flush: Arc<aggregator::Flush>,
}

pub struct Server {
    subscribe: mpsc::Sender<Watch>,
    addr: SocketAddr,
    aggregator: Option<Aggregator>,
    client_buffer: usize,
}

struct FieldVisitor {
    fields: Vec<proto::Field>,
    meta_id: proto::MetaId,
}

struct Watch(mpsc::Sender<Result<proto::tasks::TaskUpdate, tonic::Status>>);

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

        let aggregator = Aggregator::new(events, rpcs, Self::DEFAULT_FLUSH_INTERVAL);
        let flush = aggregator.flush().clone();
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
        };
        (layer, server)
    }
}

impl TasksLayer {
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
            self.flush.trigger();
        }
    }
}

impl<S> Layer<S> for TasksLayer
where
    S: Subscriber + for<'a> LookupSpan<'a>,
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
