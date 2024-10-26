use std::{
    sync::{
        atomic::{AtomicBool, Ordering::*},
        Arc,
    },
    time::{Duration, Instant},
};

use console_api as proto;
use prost::Message;
use proto::resources::resource;
use tokio::sync::{mpsc, Notify};
use tracing_core::{span::Id, Metadata};

use super::{Command, Event, Shared, Watch};
use crate::{
    stats::{self, Unsent},
    ToProto, WatchRequest,
};

mod id_data;
mod shrink;
use self::id_data::{IdData, Include};
use self::shrink::{ShrinkMap, ShrinkVec};

/// Should match tonic's (private) codec::DEFAULT_MAX_RECV_MESSAGE_SIZE
const MAX_MESSAGE_SIZE: usize = 4 * 1024 * 1024;

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

    /// Currently active RPCs streaming state events.
    state_watchers: ShrinkVec<Watch<proto::instrument::State>>,

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
    temporality: proto::instrument::Temporality,

    /// Used to anchor monotonic timestamps to a base `SystemTime`, to produce a
    /// timestamp that can be sent over the wire.
    base_time: stats::TimeAnchor,
}

#[derive(Debug, Default)]
pub(crate) struct Flush {
    pub(crate) should_flush: Notify,
    triggered: AtomicBool,
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
            state_watchers: Default::default(),
            all_metadata: Default::default(),
            new_metadata: Default::default(),
            tasks: IdData::default(),
            task_stats: IdData::default(),
            resources: IdData::default(),
            resource_stats: IdData::default(),
            async_ops: IdData::default(),
            async_op_stats: IdData::default(),
            poll_ops: Default::default(),
            temporality: proto::instrument::Temporality::Live,
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
                        proto::instrument::Temporality::Live => true,
                        proto::instrument::Temporality::Paused => false,
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
                        Some(Command::WatchState(subscription)) => {
                            self.add_state_subscription(subscription);
                        }
                        Some(Command::Pause) => {
                            self.temporality = proto::instrument::Temporality::Paused;
                        }
                        Some(Command::Resume) => {
                            self.temporality = proto::instrument::Temporality::Live;
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
            let mut counts = EventCounts::new();
            while let Some(event) = recv_now_or_never(&mut self.events) {
                match event {
                    Some(event) => {
                        counts.update(&event);
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
            tracing::debug!(
                async_resource_ops = counts.async_resource_op,
                metadatas = counts.metadata,
                poll_ops = counts.poll_op,
                resources = counts.resource,
                spawns = counts.spawn,
                total = counts.total(),
                "event channel drain loop",
            );

            if !self.state_watchers.is_empty() {
                self.publish_state();
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
        if !has_watchers {
            self.poll_ops.clear();
        }
    }

    /// Add the task subscription to the watchers after sending the first update
    fn add_instrument_subscription(&mut self, subscription: Watch<proto::instrument::Update>) {
        tracing::debug!("new instrument subscription");
        let now = Instant::now();

        let update = loop {
            let update = proto::instrument::Update {
                task_update: Some(self.task_update(Include::All)),
                resource_update: Some(self.resource_update(Include::All)),
                async_op_update: Some(self.async_op_update(Include::All)),
                now: Some(self.base_time.to_timestamp(now)),
                new_metadata: Some(proto::RegisterMetadata {
                    metadata: (*self.all_metadata).clone(),
                }),
            };
            let message_size = update.encoded_len();
            if message_size < MAX_MESSAGE_SIZE {
                // normal case
                break Some(update);
            }
            // If the grpc message is bigger than tokio-console will accept, throw away the oldest
            // inactive data and try again
            self.retention /= 2;
            self.cleanup_closed();
            tracing::debug!(
                retention = ?self.retention,
                message_size,
                max_message_size = MAX_MESSAGE_SIZE,
                "Message too big, reduced retention",
            );

            if self.retention <= self.publish_interval {
                self.retention = self.publish_interval;
                break None;
            }
        };

        match update {
            // Send the initial state
            Some(update) => {
                if !subscription.update(&update) {
                    // If sending the initial update fails, the subscription is already dead,
                    // so don't add it to `watchers`.
                    return;
                }
            }
            // User will only get updates.
            None => tracing::error!(
                min_retention = ?self.publish_interval,
                "Message too big. Start with smaller retention.",
            ),
        }

        self.watchers.push(subscription);
    }

    fn task_update(&mut self, include: Include) -> proto::tasks::TaskUpdate {
        proto::tasks::TaskUpdate {
            new_tasks: self.tasks.as_proto_list(include, &self.base_time),
            stats_update: self.task_stats.as_proto(include, &self.base_time),
            dropped_events: self.shared.dropped_tasks.swap(0, AcqRel) as u64,
        }
    }

    fn resource_update(&mut self, include: Include) -> proto::resources::ResourceUpdate {
        proto::resources::ResourceUpdate {
            new_resources: self.resources.as_proto_list(include, &self.base_time),
            stats_update: self.resource_stats.as_proto(include, &self.base_time),
            new_poll_ops: std::mem::take(&mut self.poll_ops),
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
                    .or_default()
                    .push(subscription);
            }
        }
        // If the task is not found, drop `stream_sender` which will result in a not found error
    }

    /// Add a state subscription to the watchers.
    fn add_state_subscription(&mut self, subscription: Watch<proto::instrument::State>) {
        self.state_watchers.push(subscription);
    }

    /// Publish the current state to all active state watchers.
    fn publish_state(&mut self) {
        let state = proto::instrument::State {
            temporality: self.temporality.into(),
        };
        self.state_watchers
            .retain_and_shrink(|watch| watch.update(&state));
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
                // CLI doesn't show historical poll ops, so don't save them if no-one is watching
                if self.watchers.is_empty() {
                    return;
                }
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

fn recv_now_or_never<T>(receiver: &mut mpsc::Receiver<T>) -> Option<Option<T>> {
    let waker = futures_task::noop_waker();
    let mut cx = std::task::Context::from_waker(&waker);

    match receiver.poll_recv(&mut cx) {
        std::task::Poll::Ready(opt) => Some(opt),
        std::task::Poll::Pending => None,
    }
}

/// Count of events received in each aggregator drain cycle.
struct EventCounts {
    async_resource_op: usize,
    metadata: usize,
    poll_op: usize,
    resource: usize,
    spawn: usize,
}

impl EventCounts {
    fn new() -> Self {
        Self {
            async_resource_op: 0,
            metadata: 0,
            poll_op: 0,
            resource: 0,
            spawn: 0,
        }
    }

    /// Count the event based on its variant.
    fn update(&mut self, event: &Event) {
        match event {
            Event::AsyncResourceOp { .. } => self.async_resource_op += 1,
            Event::Metadata(_) => self.metadata += 1,
            Event::PollOp { .. } => self.poll_op += 1,
            Event::Resource { .. } => self.resource += 1,
            Event::Spawn { .. } => self.spawn += 1,
        }
    }

    /// Total number of events recorded.
    fn total(&self) -> usize {
        self.async_resource_op + self.metadata + self.poll_op + self.resource + self.spawn
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
