use super::{Command, Event, Shared, Watch};
use crate::{
    stats::{self, Unsent},
    ToProto, WatchRequest,
};
use console_api as proto;
use proto::resources::resource;
use tokio::sync::{mpsc, Notify};

use futures::FutureExt;
use std::{
    sync::{
        atomic::{AtomicBool, Ordering::*},
        Arc,
    },
    time::{Duration, Instant},
};
use tracing_core::{span::Id, Metadata};

mod id_data;
mod shrink;
use self::id_data::{IdData, Include};
use self::shrink::{ShrinkMap, ShrinkVec};

/// Aggregates instrumentation traces and prepares state for the instrument
/// server.
///
/// The `Aggregator` is responsible for receiving and organizing the
/// instrumentated events and preparing the data to be served to a instrument
/// client.
pub struct Aggregator {
    /// Channel of incoming events emitted by `TaskLayer`s.
    events: mpsc::Receiver<Event>,

    /// New incoming RPCs.
    rpcs: mpsc::Receiver<Command>,

    /// The interval at which new data updates are pushed to clients.
    publish_interval: Duration,

    /// How long to keep task data after a task has completed.
    retention: Duration,

    /// Shared state, including a `Notify` that triggers a flush when the event
    /// buffer is approaching capacity.
    shared: Arc<Shared>,

    /// Currently active RPCs streaming task events.
    watchers: ShrinkVec<Watch<proto::instrument::Update>>,

    /// Currently active RPCs streaming task details events, by task ID.
    details_watchers: ShrinkMap<Id, Vec<Watch<proto::tasks::TaskDetails>>>,

    /// *All* metadata for task spans and user-defined spans that we care about.
    ///
    /// This is sent to new clients as part of the initial state.
    all_metadata: ShrinkVec<proto::register_metadata::NewMetadata>,

    /// *New* metadata that was registered since the last state update.
    ///
    /// This is emptied on every state update.
    new_metadata: Vec<proto::register_metadata::NewMetadata>,

    /// Map of task IDs to task static data.
    tasks: IdData<Task>,

    /// Map of task IDs to task stats.
    task_stats: IdData<Arc<stats::TaskStats>>,

    /// Map of resource IDs to resource static data.
    resources: IdData<Resource>,

    /// Map of resource IDs to resource stats.
    resource_stats: IdData<Arc<stats::ResourceStats>>,

    /// Map of AsyncOp IDs to AsyncOp static data.
    async_ops: IdData<AsyncOp>,

    /// Map of AsyncOp IDs to AsyncOp stats.
    async_op_stats: IdData<Arc<stats::AsyncOpStats>>,

    /// `PollOp `events that have occurred since the last update
    ///
    /// This is emptied on every state update.
    poll_ops: Vec<proto::resources::PollOp>,

    /// The time "state" of the aggregator, such as paused or live.
    temporality: Temporality,

    /// Used to anchor monotonic timestamps to a base `SystemTime`, to produce a
    /// timestamp that can be sent over the wire.
    base_time: stats::TimeAnchor,
}

#[derive(Debug, Default)]
pub(crate) struct Flush {
    pub(crate) should_flush: Notify,
    triggered: AtomicBool,
}

#[derive(Debug)]
enum Temporality {
    Live,
    Paused,
}
// Represent static data for resources
struct Resource {
    id: Id,
    is_dirty: AtomicBool,
    parent_id: Option<Id>,
    metadata: &'static Metadata<'static>,
    concrete_type: String,
    kind: resource::Kind,
    location: Option<proto::Location>,
    is_internal: bool,
}

/// Represents static data for tasks
struct Task {
    id: Id,
    is_dirty: AtomicBool,
    metadata: &'static Metadata<'static>,
    fields: Vec<proto::Field>,
    location: Option<proto::Location>,
}

struct AsyncOp {
    id: Id,
    is_dirty: AtomicBool,
    parent_id: Option<Id>,
    resource_id: Id,
    metadata: &'static Metadata<'static>,
    source: String,
}

impl Aggregator {
    pub(crate) fn new(
        events: mpsc::Receiver<Event>,
        rpcs: mpsc::Receiver<Command>,
        builder: &crate::Builder,
        shared: Arc<crate::Shared>,
        base_time: stats::TimeAnchor,
    ) -> Self {
        Self {
            shared,
            rpcs,
            publish_interval: builder.publish_interval,
            retention: builder.retention,
            events,
            watchers: Default::default(),
            details_watchers: Default::default(),
            all_metadata: Default::default(),
            new_metadata: Default::default(),
            tasks: IdData::default(),
            task_stats: IdData::default(),
            resources: IdData::default(),
            resource_stats: IdData::default(),
            async_ops: IdData::default(),
            async_op_stats: IdData::default(),
            poll_ops: Default::default(),
            temporality: Temporality::Live,
            base_time,
        }
    }

    /// Runs the aggregator.
    ///
    /// This method will start the aggregator loop and should run as long as
    /// the instrument server is running. If the instrument server stops,
    /// this future can be aborted.
    pub async fn run(mut self) {
        let mut publish = tokio::time::interval(self.publish_interval);
        loop {
            let should_send = tokio::select! {
                // if the flush interval elapses, flush data to the client
                _ = publish.tick() => {
                    match self.temporality {
                        Temporality::Live => true,
                        Temporality::Paused => false,
                    }
                }

                // triggered when the event buffer is approaching capacity
                _ = self.shared.flush.should_flush.notified() => {
                    tracing::debug!("approaching capacity; draining buffer");
                    false
                }

                // a new command from a client
                cmd = self.rpcs.recv() => {
                    match cmd {
                        Some(Command::Instrument(subscription)) => {
                            self.add_instrument_subscription(subscription);
                        },
                        Some(Command::WatchTaskDetail(watch_request)) => {
                            self.add_task_detail_subscription(watch_request);
                        },
                        Some(Command::Pause) => {
                            self.temporality = Temporality::Paused;
                        }
                        Some(Command::Resume) => {
                            self.temporality = Temporality::Live;
                        }
                        None => {
                            tracing::debug!("rpc channel closed, terminating");
                            return;
                        }
                    };

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
            let mut drained = false;
            while let Some(event) = tokio::task::unconstrained(self.events.recv()).now_or_never() {
                match event {
                    Some(event) => {
                        self.update_state(event);
                        drained = true;
                    }
                    // The channel closed, no more events will be emitted...time
                    // to stop aggregating.
                    None => {
                        tracing::debug!("event channel closed; terminating");
                        return;
                    }
                };
            }

            // flush data to clients, if there are any currently subscribed
            // watchers and we should send a new update.
            if !self.watchers.is_empty() && should_send {
                self.publish();
            }
            self.cleanup_closed();
            if drained {
                self.shared.flush.has_flushed();
            }
        }
    }

    fn cleanup_closed(&mut self) {
        // drop all closed have that has completed *and* whose final data has already
        // been sent off.
        let now = Instant::now();
        let has_watchers = !self.watchers.is_empty();
        self.tasks
            .drop_closed(&mut self.task_stats, now, self.retention, has_watchers);
        self.resources
            .drop_closed(&mut self.resource_stats, now, self.retention, has_watchers);
        self.async_ops
            .drop_closed(&mut self.async_op_stats, now, self.retention, has_watchers);
    }

    /// Add the task subscription to the watchers after sending the first update
    fn add_instrument_subscription(&mut self, subscription: Watch<proto::instrument::Update>) {
        tracing::debug!("new instrument subscription");

        let task_update = Some(self.task_update(Include::All));
        let resource_update = Some(self.resource_update(Include::All));
        let async_op_update = Some(self.async_op_update(Include::All));
        let now = Instant::now();

        let update = &proto::instrument::Update {
            task_update,
            resource_update,
            async_op_update,
            now: Some(self.base_time.to_timestamp(now)),
            new_metadata: Some(proto::RegisterMetadata {
                metadata: (*self.all_metadata).clone(),
            }),
        };

        // Send the initial state --- if this fails, the subscription is already dead
        if subscription.update(update) {
            self.watchers.push(subscription)
        }
    }

    fn task_update(&mut self, include: Include) -> proto::tasks::TaskUpdate {
        proto::tasks::TaskUpdate {
            new_tasks: self.tasks.as_proto_list(include, &self.base_time),
            stats_update: self.task_stats.as_proto(include, &self.base_time),
            dropped_events: self.shared.dropped_tasks.swap(0, AcqRel) as u64,
        }
    }

    fn resource_update(&mut self, include: Include) -> proto::resources::ResourceUpdate {
        let new_poll_ops = match include {
            Include::All => self.poll_ops.clone(),
            Include::UpdatedOnly => std::mem::take(&mut self.poll_ops),
        };
        proto::resources::ResourceUpdate {
            new_resources: self.resources.as_proto_list(include, &self.base_time),
            stats_update: self.resource_stats.as_proto(include, &self.base_time),
            new_poll_ops,
            dropped_events: self.shared.dropped_resources.swap(0, AcqRel) as u64,
        }
    }

    fn async_op_update(&mut self, include: Include) -> proto::async_ops::AsyncOpUpdate {
        proto::async_ops::AsyncOpUpdate {
            new_async_ops: self.async_ops.as_proto_list(include, &self.base_time),
            stats_update: self.async_op_stats.as_proto(include, &self.base_time),
            dropped_events: self.shared.dropped_async_ops.swap(0, AcqRel) as u64,
        }
    }

    /// Add the task details subscription to the watchers after sending the first update,
    /// if the task is found.
    fn add_task_detail_subscription(
        &mut self,
        watch_request: WatchRequest<proto::tasks::TaskDetails>,
    ) {
        let WatchRequest {
            id,
            stream_sender,
            buffer,
        } = watch_request;
        tracing::debug!(id = ?id, "new task details subscription");
        if let Some(stats) = self.task_stats.get(&id) {
            let (tx, rx) = mpsc::channel(buffer);
            let subscription = Watch(tx);
            let now = Some(self.base_time.to_timestamp(Instant::now()));
            // Send back the stream receiver.
            // Then send the initial state --- if this fails, the subscription is already dead.
            if stream_sender.send(rx).is_ok()
                && subscription.update(&proto::tasks::TaskDetails {
                    task_id: Some(id.clone().into()),
                    now,
                    poll_times_histogram: Some(stats.poll_duration_histogram()),
                    scheduled_times_histogram: Some(stats.scheduled_duration_histogram()),
                })
            {
                self.details_watchers
                    .entry(id.clone())
                    .or_insert_with(Vec::new)
                    .push(subscription);
            }
        }
        // If the task is not found, drop `stream_sender` which will result in a not found error
    }

    /// Publish the current state to all active watchers.
    ///
    /// This drops any watchers which have closed the RPC, or whose update
    /// channel has filled up.
    fn publish(&mut self) {
        let new_metadata = if !self.new_metadata.is_empty() {
            Some(proto::RegisterMetadata {
                metadata: std::mem::take(&mut self.new_metadata),
            })
        } else {
            None
        };
        let task_update = Some(self.task_update(Include::UpdatedOnly));
        let resource_update = Some(self.resource_update(Include::UpdatedOnly));
        let async_op_update = Some(self.async_op_update(Include::UpdatedOnly));

        let update = proto::instrument::Update {
            now: Some(self.base_time.to_timestamp(Instant::now())),
            new_metadata,
            task_update,
            resource_update,
            async_op_update,
        };

        self.watchers
            .retain_and_shrink(|watch: &Watch<proto::instrument::Update>| watch.update(&update));

        let stats = &self.task_stats;
        // Assuming there are much fewer task details subscribers than there are
        // stats updates, iterate over `details_watchers` and compact the map.
        self.details_watchers.retain_and_shrink(|id, watchers| {
            if let Some(task_stats) = stats.get(id) {
                let details = proto::tasks::TaskDetails {
                    task_id: Some(id.clone().into()),
                    now: Some(self.base_time.to_timestamp(Instant::now())),
                    poll_times_histogram: Some(task_stats.poll_duration_histogram()),
                    scheduled_times_histogram: Some(task_stats.scheduled_duration_histogram()),
                };
                watchers.retain(|watch| watch.update(&details));
                !watchers.is_empty()
            } else {
                false
            }
        });
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
                stats,
                fields,
                location,
            } => {
                self.tasks.insert(
                    id.clone(),
                    Task {
                        id: id.clone(),
                        is_dirty: AtomicBool::new(true),
                        metadata,
                        fields,
                        location,
                        // TODO: parents
                    },
                );

                self.task_stats.insert(id, stats);
            }

            Event::Resource {
                id,
                parent_id,
                metadata,
                kind,
                concrete_type,
                location,
                is_internal,
                stats,
            } => {
                self.resources.insert(
                    id.clone(),
                    Resource {
                        id: id.clone(),
                        is_dirty: AtomicBool::new(true),
                        parent_id,
                        kind,
                        metadata,
                        concrete_type,
                        location,
                        is_internal,
                    },
                );

                self.resource_stats.insert(id, stats);
            }

            Event::PollOp {
                metadata,
                resource_id,
                op_name,
                async_op_id,
                task_id,
                is_ready,
            } => {
                let poll_op = proto::resources::PollOp {
                    metadata: Some(metadata.into()),
                    resource_id: Some(resource_id.into()),
                    name: op_name,
                    task_id: Some(task_id.into()),
                    async_op_id: Some(async_op_id.into()),
                    is_ready,
                };

                self.poll_ops.push(poll_op);
            }

            Event::AsyncResourceOp {
                id,
                source,
                resource_id,
                metadata,
                parent_id,
                stats,
            } => {
                self.async_ops.insert(
                    id.clone(),
                    AsyncOp {
                        id: id.clone(),
                        is_dirty: AtomicBool::new(true),
                        resource_id,
                        metadata,
                        source,
                        parent_id,
                    },
                );

                self.async_op_stats.insert(id, stats);
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
        } else {
            // someone else already did it, that's fine...
        }
    }

    /// Indicates that the buffer has been successfully flushed.
    fn has_flushed(&self) {
        let _ = self
            .triggered
            .compare_exchange(true, false, AcqRel, Acquire);
    }
}

impl<T: Clone> Watch<T> {
    fn update(&self, update: &T) -> bool {
        if let Ok(reserve) = self.0.try_reserve() {
            reserve.send(Ok(update.clone()));
            true
        } else {
            false
        }
    }
}

impl ToProto for Task {
    type Output = proto::tasks::Task;

    fn to_proto(&self, _: &stats::TimeAnchor) -> Self::Output {
        proto::tasks::Task {
            id: Some(self.id.clone().into()),
            // TODO: more kinds of tasks...
            kind: proto::tasks::task::Kind::Spawn as i32,
            metadata: Some(self.metadata.into()),
            parents: Vec::new(), // TODO: implement parents nicely
            fields: self.fields.clone(),
            location: self.location.clone(),
        }
    }
}

impl Unsent for Task {
    fn take_unsent(&self) -> bool {
        self.is_dirty.swap(false, AcqRel)
    }

    fn is_unsent(&self) -> bool {
        self.is_dirty.load(Acquire)
    }
}

impl ToProto for Resource {
    type Output = proto::resources::Resource;

    fn to_proto(&self, _: &stats::TimeAnchor) -> Self::Output {
        proto::resources::Resource {
            id: Some(self.id.clone().into()),
            parent_resource_id: self.parent_id.clone().map(Into::into),
            kind: Some(self.kind.clone()),
            metadata: Some(self.metadata.into()),
            concrete_type: self.concrete_type.clone(),
            location: self.location.clone(),
            is_internal: self.is_internal,
        }
    }
}

impl Unsent for Resource {
    fn take_unsent(&self) -> bool {
        self.is_dirty.swap(false, AcqRel)
    }

    fn is_unsent(&self) -> bool {
        self.is_dirty.load(Acquire)
    }
}

impl ToProto for AsyncOp {
    type Output = proto::async_ops::AsyncOp;

    fn to_proto(&self, _: &stats::TimeAnchor) -> Self::Output {
        proto::async_ops::AsyncOp {
            id: Some(self.id.clone().into()),
            metadata: Some(self.metadata.into()),
            resource_id: Some(self.resource_id.clone().into()),
            source: self.source.clone(),
            parent_async_op_id: self.parent_id.clone().map(Into::into),
        }
    }
}

impl Unsent for AsyncOp {
    fn take_unsent(&self) -> bool {
        self.is_dirty.swap(false, AcqRel)
    }

    fn is_unsent(&self) -> bool {
        self.is_dirty.load(Acquire)
    }
}
