use console_api as proto;
use tokio::sync::mpsc;

use std::{
    ptr,
    sync::atomic::{AtomicPtr, Ordering::*},
    collections::HashMap,
    time::{SystemTime, Duration},
    mem,
    marker::PhantomData,
};
use tracing_core::{
    field::{self, Field},
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
    Layer,
};

enum Event {
    Metadata(&'static Metadata<'static>),
    Spawn {
        id: span::Id,
        metadata: &'static Metadata<'static>,
        at: SystemTime,
        fields: String
    },
    Enter { id: span::Id, at: SystemTime, },
    Exit { id: span::Id, at: SystemTime, },
    Close { id: span::Id, at: SystemTime, },
}

pub struct TasksLayer<F = DefaultFields> {
    task_meta: AtomicPtr<Metadata<'static>>,
    blocking_meta: AtomicPtr<Metadata<'static>>,
    tx: mpsc::Sender<Event>,
    _f: PhantomData<fn(F)>,
}

pub struct Server {
    events: mpsc::Receiver<Event>,
}

struct Watch(mpsc::Sender<proto::tasks::TaskUpdate>);

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

impl<F> TasksLayer<F> {
    #[inline(always)]
    fn is_spawn(&self, meta: &'static Metadata<'static>) -> bool {
        ptr::eq(self.task_meta.load(Relaxed), meta as *const _ as *mut _)
            // || ptr::eq(self.blocking_meta.load(Relaxed), meta as *const _ as *mut _)
    }
}

impl<S, F> Layer<S> for TasksLayer<F>
where
    S: Subscriber,
    F: for<'writer> FormatFields<'writer> + 'static,
{
    fn register_callsite(&self, meta: &'static Metadata<'static>) -> subscriber::Interest {
        if meta.target() == "tokio::task" && meta.name() == "task" {
            if meta.fields().iter().any(|f| f.name() == "function") {
                self.blocking_meta.compare_and_swap(
                    ptr::null_mut(),
                    meta as *const _ as *mut _,
                    AcqRel,
                );
            } else {
                self.task_meta.compare_and_swap(
                    ptr::null_mut(),
                    meta as *const _ as *mut _,
                    AcqRel,
                );
            }
        }

        let _ = self.tx.blocking_send(Event::Metadata(meta));

        subscriber::Interest::always()
    }

    fn new_span(&self, attrs: &span::Attributes<'_>, id: &span::Id, cx: Context<'_, S>) {
        let metadata = attrs.metadata();
        if self.is_spawn(metadata) {
            let at = SystemTime::now();
            let _ = self.tx.blocking_send(Event::Spawn {
                id, at, metadata, fields: String::new() // TODO(eliza): format fields
            }));
        }
    }

    fn on_enter(&self, id: &span::Id, cx: Context<'_, S>) {
        todo!("track only spawned tasks")
    }

    fn on_exit(&self, id: &span::Id, cx: Context<'_, S>) {
        todo!("track only spawned tasks")
    }

    fn on_close(&self, id: &span::Id, cx: Context<'_, S>) {
        todo!("track only spawned tasks")
    }
}


impl Server {
    async fn run_bg(
        mut events: mpsc::Receiver<Event>,
        mut rpcs: mpsc::Receiver<Watch>,
        flush_interval: Duration,
    ) {
        let mut flush = tokio::time::interval(flush_interval);
        let mut watches = Vec::new();
        let mut metadata = Vec::new();
        let mut new_metadata = Vec::new();
        let mut tasks = TaskData::default();
        let mut stats = TaskData::default();
        let mut completed_spans = Vec::new();

        tokio::select! { biased;
            _ = flush.tick() => {
                let new_metadata = if !new_metadata.is_empty() {
                    Some(proto::RegisterMetadata {
                        new_metadata: mem::replace(&mut new_metadata, Vec::new()),
                    })
                } else {
                    None
                };
                let new_tasks = tasks.since_last_update().map(|(id, task)| {
                    task.to_proto(id)
                }).collect();
                let now = SystemTime::now();
                let stats_update = stats.since_last_update().map(|(id, stats)| {
                    (id.into_u64(), stats.to_proto(now))
                }).collect();
                let update = proto::tasks::TaskUpdate {
                    new_metadata,
                    new_tasks,
                    stats_update,
                    completed_spans: mem::replace(&mut completed_spans, Vec::new());
                };
                watches.retain(|watch| watch.update(&update));
            }
            event = events.recv() => {
                let event = match event {
                    Some(event) => event,
                    None => return,
                };

                // do state update
                match event {
                    Event::Metadata(meta) => {
                        metadata.push(meta.into());
                        new_metadata.push(meta.into());
                    },
                    Event::Spawn { id, metadata, at, fields } => {
                        tasks.insert(id.clone(), Task {
                            metadata,
                            fields,
                            // TODO: parents
                        });
                        stats.insert(id, Stats { polls: 0, created_at: Some(at), ..Default::default() })
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
                    },

                    Event::Close { id, at } => {
                        stats.update_or_default(id).closed_at = Some(at);
                        completed_spans.push(id.into());
                    }
                }
            }
            subscription = rpcs.recv() => {
                let new_tasks = tasks.all().map(|(id, task)| {
                    task.to_proto(id)
                }).collect();
                let now = SystemTime::now();
                let stats_update = stats.all().map(|(id, stats)| {
                    (id.into_u64(), stats.to_proto(now))
                }).collect();
                // Send the initial state --- if this fails, the subscription is
                // already dead.
                if subscription.update(proto::tasks::TaskUpdate {
                    new_metadata: Some(proto::RegisterMetadata {
                        new_metadata: metadata.clone(),
                    }),
                    new_tasks,
                    stats_update,
                    ..Default::default()
                }) {
                    watches.push(subscription)
                }
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

    fn update(&mut self, id: span::Id) -> Option<Updating<'_, T>>
    {
        Updating(self.data.get_mut(id))
    }

    fn insert(&mut self, id: span::Id, data: T) -> Updating<'_, T> {
        self.data.insert(id, (data, true))
    }


    fn since_last_update(&mut self) -> impl Iterator<Item = (span::Id, T)> {
        self.data.iter_mut().filter_map(|(id, (data, dirty)| {
            if dirty {
                dirty = false;
                Some((id, data))
            } else {
                None
            }
        })
    }

    fn all(&self) -> impl Iterator<Item = (span::Id, T)> {
        self.data.iter().map(|(id, (data, dirty)| (id, data)))
    }
}

struct Updating<'a, T>(&'a mut (T, bool));

impl<'a, T> Deref<T> for Updating<'a, T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        &self.0 .0
    }
}

impl<'a, T> DerefMut<T> for Updating<'a, T> {
    fn deref(&mut self) -> &mut Self::Target {
        &mut self.0 .0
    }
}

impl<'a, T> Drop for Updating<'a, T> {
    fn drop(&mut self) -> Self {
        self.0 .1 = true;
    }
}

impl Watch {
    fn update(&self, update: impl ToOwned<Owned = proto::tasks::TaskUpdate>) -> bool {
        if let Ok(reserve) = self.0.try_reserve() {
            reserve.send(Ok(update.to_owned()));
            true
        } else {
            false
        }
    }
}

impl Stats {
    fn total_time(&self, now: SystemTime) -> Option<Duration> {
        let now = self.closed_at.unwrap_or(now);
        self.created_at.and_then(|then| now.duration_since(then).ok())
    }

    fn to_proto(&self, now: SystemTime) -> proto::tasks::Stats {
        proto::tasks::Stats {
            polls: self.polls,
            created_at: self.created_at.map(Into::into),
            first_poll: self.created_at.map(Into::into),
            last_poll: self.created_at.map(Into::into),
            busy_time: self.busy_time.into(),
            total_time: self.total_time(now),
        };
    }
}


impl Task {
    fn to_proto(&self, id: span::Id) -> proto::tasks::Task {
        proto::tasks::Task {
            id: Some(id.into()),
            // TODO: more kinds of tasks...
            kind: proto::tasks::task::Kind::Spawn as i32,
            fields: Some(proto::tasks::task::Fields::StringFields(self.fields.clone())),
            meta_id: Some(self.metadata.into()),
            parents: Vec::new() // TODO: implement parents nicely
        }
    }
}