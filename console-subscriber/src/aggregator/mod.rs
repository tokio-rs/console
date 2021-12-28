use super::{AttributeUpdate, AttributeUpdateOp, Command, Event, Shared, Watch};
use crate::{
    record::Recorder,
    stats::{self, Unsent},
    ToProto, WatchRequest,
};
use console_api as proto;
use proto::resources::resource;
use proto::Attribute;
use tokio::sync::{mpsc, Notify};

use futures::FutureExt;
use std::{
    collections::hash_map::{Entry, HashMap},
    convert::TryInto,
    sync::{
        atomic::{AtomicBool, Ordering::*},
        Arc,
    },
    time::{Duration, SystemTime},
};
use tracing_core::{span::Id, Metadata};

use hdrhistogram::{
    serialization::{Serializer, V2SerializeError, V2Serializer},
    Histogram,
};

mod id_data;
mod shrink;
use self::id_data::{IdData, Include};
use self::shrink::{ShrinkMap, ShrinkVec};

pub(crate) struct Aggregator {
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

    /// *All* PollOp events for AsyncOps on Resources.
    ///
    /// This is sent to new clients as part of the initial state.
    // TODO: drop the poll ops for async ops that have been dropped
    all_poll_ops: ShrinkVec<proto::resources::PollOp>,

    /// *New* PollOp events that whave occurred since the last update
    ///
    /// This is emptied on every state update.
    new_poll_ops: Vec<proto::resources::PollOp>,

    /// A sink to record all events to a file.
    recorder: Option<Recorder>,

    /// The time "state" of the aggregator, such as paused or live.
    temporality: Temporality,
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
    inherit_child_attrs: bool,
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
    inherit_child_attrs: bool,
}

impl Aggregator {
    pub(crate) fn new(
        events: mpsc::Receiver<Event>,
        rpcs: mpsc::Receiver<Command>,
        builder: &crate::Builder,
        shared: Arc<crate::Shared>,
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
            all_poll_ops: Default::default(),
            new_poll_ops: Default::default(),
            recorder: builder
                .recording_path
                .as_ref()
                .map(|path| Recorder::new(path).expect("creating recorder")),
            temporality: Temporality::Live,
        }
    }

    pub(crate) async fn run(mut self) {
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
            while let Some(event) = self.events.recv().now_or_never() {
                match event {
                    Some(event) => {
                        // always be recording...
                        if let Some(ref recorder) = self.recorder {
                            recorder.record(&event);
                        }
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
        let now = SystemTime::now();
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
        let now = SystemTime::now();
        // Send the initial state --- if this fails, the subscription is already dead
        let update = &proto::instrument::Update {
            task_update: Some(proto::tasks::TaskUpdate {
                new_tasks: self
                    .tasks
                    .all()
                    .map(|(_, value)| value.to_proto())
                    .collect(),
                stats_update: self.task_stats.as_proto(Include::All),
                dropped_events: self.shared.dropped_tasks.swap(0, AcqRel) as u64,
            }),
            resource_update: Some(proto::resources::ResourceUpdate {
                new_resources: self
                    .resources
                    .all()
                    .map(|(_, value)| value.to_proto())
                    .collect(),
                stats_update: self.resource_stats.as_proto(Include::All),
                new_poll_ops: (*self.all_poll_ops).clone(),
                dropped_events: self.shared.dropped_resources.swap(0, AcqRel) as u64,
            }),
            async_op_update: Some(proto::async_ops::AsyncOpUpdate {
                new_async_ops: self
                    .async_ops
                    .all()
                    .map(|(_, value)| value.to_proto())
                    .collect(),
                stats_update: self.async_op_stats.as_proto(Include::All),
                dropped_events: self.shared.dropped_async_ops.swap(0, AcqRel) as u64,
            }),
            now: Some(now.into()),
            new_metadata: Some(proto::RegisterMetadata {
                metadata: (*self.all_metadata).clone(),
            }),
        };

        if subscription.update(update) {
            self.watchers.push(subscription)
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
            let now = SystemTime::now();
            // Send back the stream receiver.
            // Then send the initial state --- if this fails, the subscription is already dead.
            if stream_sender.send(rx).is_ok()
                && subscription.update(&proto::tasks::TaskDetails {
                    task_id: Some(id.clone().into()),
                    now: Some(now.into()),
                    poll_times_histogram: None, // TODO(eliza): put back
                                                // poll_times_histogram: serialize_histogram(&stats.poll_times_histogram).ok(),
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

        let new_poll_ops = std::mem::take(&mut self.new_poll_ops);

        let now = SystemTime::now();
        let update = proto::instrument::Update {
            now: Some(now.into()),
            new_metadata,
            task_update: Some(proto::tasks::TaskUpdate {
                new_tasks: self
                    .tasks
                    .since_last_update()
                    .map(|(_, value)| value.to_proto())
                    .collect(),
                stats_update: self.task_stats.as_proto(Include::UpdatedOnly),

                dropped_events: self.shared.dropped_tasks.swap(0, AcqRel) as u64,
            }),
            resource_update: Some(proto::resources::ResourceUpdate {
                new_resources: self
                    .resources
                    .since_last_update()
                    .map(|(_, value)| value.to_proto())
                    .collect(),
                stats_update: self.resource_stats.as_proto(Include::UpdatedOnly),
                new_poll_ops,

                dropped_events: self.shared.dropped_resources.swap(0, AcqRel) as u64,
            }),
            async_op_update: Some(proto::async_ops::AsyncOpUpdate {
                new_async_ops: self
                    .async_ops
                    .since_last_update()
                    .map(|(_, value)| value.to_proto())
                    .collect(),
                stats_update: self.async_op_stats.as_proto(Include::UpdatedOnly),

                dropped_events: self.shared.dropped_async_ops.swap(0, AcqRel) as u64,
            }),
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
                    now: Some(now.into()),
                    // poll_times_histogram: serialize_histogram(&task_stats.poll_times_histogram)
                    //     .ok(),
                    poll_times_histogram: None, // TODO(eliza): put back
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
                        is_dirty: AtomicBool::new(false),
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
                inherit_child_attrs,
                stats,
            } => {
                self.resources.insert(
                    id.clone(),
                    Resource {
                        id: id.clone(),
                        is_dirty: AtomicBool::new(false),
                        parent_id,
                        kind,
                        metadata,
                        concrete_type,
                        location,
                        is_internal,
                        inherit_child_attrs,
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
                // TODO(eliza): put back

                // let mut async_op_stats = self.async_op_stats.update_or_default(async_op_id.clone());
                // async_op_stats.task_id.get_or_insert(task_id.clone());

                // let poll_op = proto::resources::PollOp {
                //     metadata: Some(metadata.into()),
                //     resource_id: Some(resource_id.into()),
                //     name: op_name,
                //     task_id: Some(task_id.into()),
                //     async_op_id: Some(async_op_id.into()),
                //     is_ready,
                // };

                // self.all_poll_ops.push(poll_op.clone());
                // self.new_poll_ops.push(poll_op);
            }

            Event::StateUpdate {
                update_id,
                update_type,
                update,
                ..
            } => {
                // let update_id = self.ids.id_for(update_id);
                //         let mut to_update = vec![(update_id, update_type.clone())];

                //         fn update_entry(e: Entry<'_, FieldKey, Attribute>, upd: &AttributeUpdate) {
                //             e.and_modify(|attr| update_attribute(attr, upd))
                //                 .or_insert_with(|| upd.clone().into());
                //         }

                //         match update_type {
                //             UpdateType::Resource => {
                //                 if let Some(parent) = self
                //                     .resources
                //                     .get(&update_id)
                //                     .and_then(|r| self.resources.get(r.parent_id.as_ref()?))
                //                     .filter(|parent| parent.inherit_child_attrs)
                //                 {
                //                     to_update.push((parent.id, UpdateType::Resource));
                //                 }
                //             }
                //             UpdateType::AsyncOp => {
                //                 if let Some(parent) = self
                //                     .async_ops
                //                     .get(&update_id)
                //                     .and_then(|r| self.async_ops.get(r.parent_id.as_ref()?))
                //                     .filter(|parent| parent.inherit_child_attrs)
                //                 {
                //                     to_update.push((parent.id, UpdateType::AsyncOp));
                //                 }
                //             }
                //         }

                //         for (update_id, update_type) in to_update {
                //             let field_name = match update.field.name.as_ref() {
                //                 Some(name) => name.clone(),
                //                 None => {
                //                     tracing::warn!(?update.field, "field missing name, skipping...");
                //                     return;
                //                 }
                //             };

                //             let upd_key = FieldKey {
                //                 update_id,
                //                 field_name,
                //             };

                //             match update_type {
                //                 UpdateType::Resource => {
                //                     let mut stats = self.resource_stats.update(&update_id);
                //                     let entry = stats.as_mut().map(|s| s.attributes.entry(upd_key));
                //                     if let Some(entry) = entry {
                //                         update_entry(entry, &update);
                //                     }
                //                 }
                //                 UpdateType::AsyncOp => {
                //                     let mut stats = self.async_op_stats.update(&update_id);
                //                     let entry = stats.as_mut().map(|s| s.attributes.entry(upd_key));
                //                     if let Some(entry) = entry {
                //                         update_entry(entry, &update);
                //                     }
                //                 }
                //             };
                //         }
            }

            Event::AsyncResourceOp {
                id,
                source,
                resource_id,
                metadata,
                parent_id,
                inherit_child_attrs,
                stats,
            } => {
                self.async_ops.insert(
                    id.clone(),
                    AsyncOp {
                        id: id.clone(),
                        is_dirty: AtomicBool::new(false),
                        resource_id,
                        metadata,
                        source,
                        parent_id,
                        inherit_child_attrs,
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

    fn to_proto(&self) -> Self::Output {
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

    fn to_proto(&self) -> Self::Output {
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

    fn to_proto(&self) -> Self::Output {
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

impl From<AttributeUpdate> for Attribute {
    fn from(upd: AttributeUpdate) -> Self {
        Attribute {
            field: Some(upd.field),
            unit: upd.unit,
        }
    }
}

fn serialize_histogram(histogram: &Histogram<u64>) -> Result<Vec<u8>, V2SerializeError> {
    let mut serializer = V2Serializer::new();
    let mut buf = Vec::new();
    serializer.serialize(histogram, &mut buf)?;
    Ok(buf)
}

fn update_attribute(attribute: &mut Attribute, update: &AttributeUpdate) {
    use proto::field::Value::*;
    let attribute_val = attribute.field.as_mut().and_then(|a| a.value.as_mut());
    let update_val = update.field.value.clone();
    let update_name = update.field.name.clone();
    match (attribute_val, update_val) {
        (Some(BoolVal(v)), Some(BoolVal(upd))) => *v = upd,

        (Some(StrVal(v)), Some(StrVal(upd))) => *v = upd,

        (Some(DebugVal(v)), Some(DebugVal(upd))) => *v = upd,

        (Some(U64Val(v)), Some(U64Val(upd))) => match update.op {
            Some(AttributeUpdateOp::Add) => *v = v.saturating_add(upd),

            Some(AttributeUpdateOp::Sub) => *v = v.saturating_sub(upd),

            Some(AttributeUpdateOp::Override) => *v = upd,

            None => tracing::warn!(
                "numeric attribute update {:?} needs to have an op field",
                update_name
            ),
        },

        (Some(I64Val(v)), Some(I64Val(upd))) => match update.op {
            Some(AttributeUpdateOp::Add) => *v = v.saturating_add(upd),

            Some(AttributeUpdateOp::Sub) => *v = v.saturating_sub(upd),

            Some(AttributeUpdateOp::Override) => *v = upd,

            None => tracing::warn!(
                "numeric attribute update {:?} needs to have an op field",
                update_name
            ),
        },

        (val, update) => {
            tracing::warn!(
                "attribute {:?} cannot be updated by update {:?}",
                val,
                update
            );
        }
    }
}
