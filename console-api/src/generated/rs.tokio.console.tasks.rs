/// A task state update.
///
/// Each `TaskUpdate` contains any task data that has changed since the last
/// update. This includes:
/// - any new tasks that were spawned since the last update
/// - the current stats for any task whose stats changed since the last update
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct TaskUpdate {
    /// A list of new tasks that were spawned since the last `TaskUpdate` was
    /// sent.
    ///
    /// If this is empty, no new tasks were spawned.
    #[prost(message, repeated, tag="1")]
    pub new_tasks: ::prost::alloc::vec::Vec<Task>,
    /// Any task stats that have changed since the last update.
    ///
    /// This is a map of task IDs (64-bit unsigned integers) to task stats. If a
    /// task's ID is not included in this map, then its stats have *not* changed
    /// since the last `TaskUpdate` in which they were present. If a task's ID
    /// *is* included in this map, the corresponding value represents a complete
    /// snapshot of that task's stats at in the current time window.
    #[prost(map="uint64, message", tag="3")]
    pub stats_update: ::std::collections::HashMap<u64, Stats>,
    /// A count of how many task events (e.g. polls, spawns, etc) were not
    /// recorded because the application's event buffer was at capacity.
    ///
    /// If everything is working normally, this should be 0. If it is greater
    /// than 0, that may indicate that some data is missing from this update, and
    /// it may be necessary to increase the number of events buffered by the
    /// application to ensure that data loss is avoided.
    ///
    /// If the application's instrumentation ensures reliable delivery of events,
    /// this will always be 0.
    #[prost(uint64, tag="4")]
    pub dropped_events: u64,
}
/// A task details update
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct TaskDetails {
    /// The task's ID which the details belong to.
    #[prost(message, optional, tag="1")]
    pub task_id: ::core::option::Option<super::common::Id>,
    /// The timestamp for when the update to the task took place.
    #[prost(message, optional, tag="2")]
    pub now: ::core::option::Option<::prost_types::Timestamp>,
    /// A histogram of task poll durations.
    ///
    /// This is either:
    /// - the raw binary representation of a HdrHistogram.rs `Histogram`
    ///    serialized to binary in the V2 format (legacy)
    /// - a binary histogram plus details on outliers (current)
    #[prost(oneof="task_details::PollTimesHistogram", tags="3, 4")]
    pub poll_times_histogram: ::core::option::Option<task_details::PollTimesHistogram>,
}
/// Nested message and enum types in `TaskDetails`.
pub mod task_details {
    /// A histogram of task poll durations.
    ///
    /// This is either:
    /// - the raw binary representation of a HdrHistogram.rs `Histogram`
    ///    serialized to binary in the V2 format (legacy)
    /// - a binary histogram plus details on outliers (current)
    #[derive(Clone, PartialEq, ::prost::Oneof)]
    pub enum PollTimesHistogram {
        /// HdrHistogram.rs `Histogram` serialized to binary in the V2 format
        #[prost(bytes, tag="3")]
        LegacyHistogram(::prost::alloc::vec::Vec<u8>),
        /// A histogram plus additional data.
        #[prost(message, tag="4")]
        Histogram(super::DurationHistogram),
    }
}
/// Data recorded when a new task is spawned.
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Task {
    /// The task's ID.
    ///
    /// This uniquely identifies this task across all *currently live* tasks.
    /// When the task's stats change, or when the task completes, it will be
    /// identified by this ID; if the client requires additional information
    /// included in the `Task` message, it should store that data and access it
    /// by ID.
    #[prost(message, optional, tag="1")]
    pub id: ::core::option::Option<super::common::Id>,
    /// The numeric ID of the task's `Metadata`.
    ///
    /// This identifies the `Metadata` that describes the `tracing` span
    /// corresponding to this task. The metadata for this ID will have been sent
    /// in a prior `RegisterMetadata` message.
    #[prost(message, optional, tag="2")]
    pub metadata: ::core::option::Option<super::common::MetaId>,
    /// The category of task this task belongs to.
    #[prost(enumeration="task::Kind", tag="3")]
    pub kind: i32,
    /// A list of `Field` objects attached to this task.
    #[prost(message, repeated, tag="4")]
    pub fields: ::prost::alloc::vec::Vec<super::common::Field>,
    /// An ordered list of span IDs corresponding to the `tracing` span context
    /// in which this task was spawned.
    ///
    /// The first span ID in this list is the immediate parent, followed by that
    /// span's parent, and so on. The final ID is the root span of the current
    /// trace.
    ///
    /// If this is empty, there were *no* active spans when the task was spawned.
    ///
    /// These IDs may correspond to `tracing` spans which are *not* tasks, if
    /// additional trace data is being collected.
    #[prost(message, repeated, tag="5")]
    pub parents: ::prost::alloc::vec::Vec<super::common::SpanId>,
    /// The location in code where the task was spawned.
    #[prost(message, optional, tag="6")]
    pub location: ::core::option::Option<super::common::Location>,
}
/// Nested message and enum types in `Task`.
pub mod task {
    /// The category of task this task belongs to.
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
    #[repr(i32)]
    pub enum Kind {
        /// A task spawned using a runtime's standard asynchronous task spawning
        /// operation (such as `tokio::task::spawn`).
        Spawn = 0,
        /// A task spawned via a runtime's blocking task spawning operation
        /// (such as `tokio::task::spawn_blocking`).
        Blocking = 1,
    }
    impl Kind {
        /// String value of the enum field names used in the ProtoBuf definition.
        ///
        /// The values are not transformed in any way and thus are considered stable
        /// (if the ProtoBuf definition does not change) and safe for programmatic use.
        pub fn as_str_name(&self) -> &'static str {
            match self {
                Kind::Spawn => "SPAWN",
                Kind::Blocking => "BLOCKING",
            }
        }
    }
}
/// Task performance statistics.
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Stats {
    /// Timestamp of when the task was spawned.
    #[prost(message, optional, tag="1")]
    pub created_at: ::core::option::Option<::prost_types::Timestamp>,
    /// Timestamp of when the task was dropped.
    #[prost(message, optional, tag="2")]
    pub dropped_at: ::core::option::Option<::prost_types::Timestamp>,
    /// The total number of times this task has been woken over its lifetime.
    #[prost(uint64, tag="3")]
    pub wakes: u64,
    /// The total number of times this task's waker has been cloned.
    #[prost(uint64, tag="4")]
    pub waker_clones: u64,
    /// The total number of times this task's waker has been dropped.
    #[prost(uint64, tag="5")]
    pub waker_drops: u64,
    /// The timestamp of the most recent time this task has been woken.
    ///
    /// If this is `None`, the task has not yet been woken.
    #[prost(message, optional, tag="6")]
    pub last_wake: ::core::option::Option<::prost_types::Timestamp>,
    /// Contains task poll statistics.
    #[prost(message, optional, tag="7")]
    pub poll_stats: ::core::option::Option<super::common::PollStats>,
    /// The total number of times this task has woken itself.
    #[prost(uint64, tag="8")]
    pub self_wakes: u64,
    /// The total duration this task was scheduled prior to being polled, summed
    /// across all poll cycles.
    ///
    /// Note that this includes only polls that have started, and does not
    /// reflect any scheduled state where the task hasn't yet been polled.
    /// Subtracting both `busy_time` (from the task's `PollStats`) and
    /// `scheduled_time` from the total lifetime of the task results in the
    /// amount of time it spent unable to progress because it was waiting on 
    /// some resource.
    #[prost(message, optional, tag="9")]
    pub scheduled_time: ::core::option::Option<::prost_types::Duration>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct DurationHistogram {
    /// HdrHistogram.rs `Histogram` serialized to binary in the V2 format
    #[prost(bytes="vec", tag="1")]
    pub raw_histogram: ::prost::alloc::vec::Vec<u8>,
    /// The histogram's maximum value.
    #[prost(uint64, tag="2")]
    pub max_value: u64,
    /// The number of outliers which have exceeded the histogram's maximum value.
    #[prost(uint64, tag="3")]
    pub high_outliers: u64,
    /// The highest recorded outlier. This is only present if `high_outliers` is
    /// greater than zero.
    #[prost(uint64, optional, tag="4")]
    pub highest_outlier: ::core::option::Option<u64>,
}
