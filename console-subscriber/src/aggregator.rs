use super::{Event, Watch};
use console_api as proto;
use tokio::sync::{mpsc, Notify};

use futures::FutureExt;
use std::{
    collections::HashMap,
    mem,
    ops::{Deref, DerefMut},
    sync::{
        atomic::{AtomicBool, Ordering::*},
        Arc,
    },
    time::{Duration, SystemTime},
};
use tracing_core::{span, Metadata};
pub(crate) struct Aggregator {
    /// Channel of incoming events emitted by `TaskLayer`s.
    events: mpsc::Receiver<Event>,

    /// New incoming `WatchTasks` RPCs.
    rpcs: mpsc::Receiver<Watch>,

    /// The interval at which new data updates are pushed to clients.
    flush_interval: Duration,

    /// Triggers a flush when the event buffer is approaching capacity.
    flush_capacity: Arc<Flush>,

    // Currently active RPCs streaming task events.
    watchers: Vec<Watch>,

    /// *All* metadata for task spans and user-defined spans that we care about.
    ///
    /// This is sent to new clients as part of the initial state.
    all_metadata: Vec<proto::register_metadata::NewMetadata>,

    /// *New* metadata that was registered since the last state update.
    ///
    /// This is emptied on every state update.
    new_metadata: Vec<proto::register_metadata::NewMetadata>,

    /// Map of task IDs to task static data.
    tasks: TaskData<Task>,

    /// Map of task IDs to task stats.
    stats: TaskData<Stats>,
}

#[derive(Debug)]
pub(crate) struct Flush {
    pub(crate) should_flush: Notify,
    pub(crate) triggered: AtomicBool,
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

impl Aggregator {
    pub(crate) fn new(
        events: mpsc::Receiver<Event>,
        rpcs: mpsc::Receiver<Watch>,
        flush_interval: Duration,
    ) -> Self {
        Self {
            flush_capacity: Arc::new(Flush {
                should_flush: Notify::new(),
                triggered: AtomicBool::new(false),
            }),
            rpcs,
            flush_interval,
            events,
            watchers: Vec::new(),
            all_metadata: Vec::new(),
            new_metadata: Vec::new(),
            tasks: TaskData {
                data: HashMap::<span::Id, (Task, bool)>::new(),
            },
            stats: TaskData::default(),
        }
    }

    pub(crate) fn flush(&self) -> &Arc<Flush> {
        &self.flush_capacity
    }

    pub(crate) async fn run(mut self) {
        let mut flush = tokio::time::interval(self.flush_interval);
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

                // a new client has started watching!
                subscription = self.rpcs.recv() => {
                    if let Some(subscription) = subscription {
                        tracing::debug!("new subscription");
                        let new_tasks = self.tasks.all().map(|(id, task)| {
                            task.to_proto(id.clone())
                        }).collect();
                        let now = SystemTime::now();
                        let stats_update = self.stats.all().map(|(id, stats)| {
                            (id.into_u64(), stats.to_proto())
                        }).collect();
                        // Send the initial state --- if this fails, the subscription is
                        // already dead.
                        if subscription.update(&proto::tasks::TaskUpdate {
                            new_metadata: Some(proto::RegisterMetadata {
                                metadata: self.all_metadata.clone(),
                            }),
                            new_tasks,
                            stats_update,
                            now: Some(now.into()),
                        }) {
                            self.watchers.push(subscription)
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
                match event {
                    Some(event) => self.update_state(event),
                    // The channel closed, no more events will be emitted...time
                    // to stop aggregating.
                    None => {
                        tracing::debug!("event channel closed; terminating");
                        return;
                    }
                };
            }

            // flush data to clients
            if should_send {
                self.publish();
            }
        }
    }

    /// Publish the current state to all active watchers.
    ///
    /// This drops any watchers which have closed the RPC, or whose update
    /// channel has filled up.
    fn publish(&mut self) {
        let new_metadata = if !self.new_metadata.is_empty() {
            Some(proto::RegisterMetadata {
                metadata: mem::replace(&mut self.new_metadata, Vec::new()),
            })
        } else {
            None
        };
        let new_tasks = self
            .tasks
            .since_last_update()
            .map(|(id, task)| task.to_proto(id.clone()))
            .collect();
        let now = SystemTime::now();
        let stats_update = self
            .stats
            .since_last_update()
            .map(|(id, stats)| (id.into_u64(), stats.to_proto()))
            .collect();
        let update = proto::tasks::TaskUpdate {
            new_metadata,
            new_tasks,
            stats_update,
            now: Some(now.into()),
        };
        self.watchers.retain(|watch: &Watch| watch.update(&update));
    }

    /// Update the current state with data from a single event.
    fn update_state(&mut self, event: Event) {
        // do state update
        match event {
            Event::Metadata(meta) => {
                self.all_metadata.push(meta.into());
                self.new_metadata.push(meta.into());
            }
            Event::Spawn {
                id,
                metadata,
                at,
                fields,
            } => {
                self.tasks.insert(
                    id.clone(),
                    Task {
                        metadata,
                        fields,
                        // TODO: parents
                    },
                );
                self.stats.insert(
                    id,
                    Stats {
                        polls: 0,
                        created_at: Some(at),
                        ..Default::default()
                    },
                );
            }
            Event::Enter { id, at } => {
                let mut stats = self.stats.update_or_default(id);
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
                let mut stats = self.stats.update_or_default(id);
                stats.current_polls -= 1;
                if stats.current_polls == 0 {
                    if let Some(last_poll) = stats.last_poll {
                        stats.busy_time += at.duration_since(last_poll).unwrap();
                    }
                }
            }

            Event::Close { id, at } => {
                self.stats.update_or_default(id).closed_at = Some(at);
            }
        }
    }
}

// ==== impl Flush ===

impl Flush {
    pub(crate) fn trigger(&self) {
        if self
            .triggered
            .compare_exchange(false, true, AcqRel, Acquire)
            .is_ok()
        {
            self.should_flush.notify_one();
            tracing::trace!("flush triggered");
        } else {
            // someone else already did it, that's fine...
            tracing::trace!("flush already triggered");
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
    fn total_time(&self) -> Option<Duration> {
        self.closed_at.and_then(|end| {
            self.created_at
                .and_then(|start| end.duration_since(start).ok())
        })
    }

    fn to_proto(&self) -> proto::tasks::Stats {
        proto::tasks::Stats {
            polls: self.polls,
            created_at: self.created_at.map(Into::into),
            first_poll: self.first_poll.map(Into::into),
            last_poll: self.last_poll.map(Into::into),
            busy_time: Some(self.busy_time.into()),
            total_time: self.total_time().map(Into::into),
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
