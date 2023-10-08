/// An `AsyncOp` state update.
///
/// This includes a list of any new async ops, and updates to the associated statistics
/// for any async ops that have changed since the last update.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct AsyncOpUpdate {
    /// A list of new async operations that were created since the last `AsyncOpUpdate`
    /// was sent. Note that the fact that an async operation has been created
    /// does not mean that is has been polled or is being polled. This information
    /// is reflected in the `Stats` of the operation.
    #[prost(message, repeated, tag = "1")]
    pub new_async_ops: ::prost::alloc::vec::Vec<AsyncOp>,
    /// Any async op stats that have changed since the last update.
    #[prost(map = "uint64, message", tag = "2")]
    pub stats_update: ::std::collections::HashMap<u64, Stats>,
    /// A count of how many async op events (e.g. polls, creation, etc) were not
    /// recorded because the application's event buffer was at capacity.
    ///
    /// If everything is working normally, this should be 0. If it is greater
    /// than 0, that may indicate that some data is missing from this update, and
    /// it may be necessary to increase the number of events buffered by the
    /// application to ensure that data loss is avoided.
    ///
    /// If the application's instrumentation ensures reliable delivery of events,
    /// this will always be 0.
    #[prost(uint64, tag = "3")]
    pub dropped_events: u64,
}
/// An async operation.
///
/// An async operation is an operation that is associated with a resource
/// This could, for example, be a read or write on a TCP stream, or a receive operation on
/// a channel.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct AsyncOp {
    /// The async op's ID.
    ///
    /// This uniquely identifies this op across all *currently live*
    /// ones.
    #[prost(message, optional, tag = "1")]
    pub id: ::core::option::Option<super::common::Id>,
    /// The numeric ID of the op's `Metadata`.
    ///
    /// This identifies the `Metadata` that describes the `tracing` span
    /// corresponding to this async op. The metadata for this ID will have been sent
    /// in a prior `RegisterMetadata` message.
    #[prost(message, optional, tag = "2")]
    pub metadata: ::core::option::Option<super::common::MetaId>,
    /// The source of this async operation. Most commonly this should be the name
    /// of the method where the instantiation of this op has happened.
    #[prost(string, tag = "3")]
    pub source: ::prost::alloc::string::String,
    /// The ID of the parent async op.
    ///
    /// This field is only set if this async op was created while inside of another
    /// async op.  For example, `tokio::sync`'s `Mutex::lock` internally calls
    /// `Semaphore::acquire`.
    ///
    /// This field can be empty; if it is empty, this async op is not a child of another
    /// async op.
    #[prost(message, optional, tag = "4")]
    pub parent_async_op_id: ::core::option::Option<super::common::Id>,
    /// The resources's ID.
    #[prost(message, optional, tag = "5")]
    pub resource_id: ::core::option::Option<super::common::Id>,
}
/// Statistics associated with a given async operation.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Stats {
    /// Timestamp of when the async op has been created.
    #[prost(message, optional, tag = "1")]
    pub created_at: ::core::option::Option<::prost_types::Timestamp>,
    /// Timestamp of when the async op was dropped.
    #[prost(message, optional, tag = "2")]
    pub dropped_at: ::core::option::Option<::prost_types::Timestamp>,
    /// The Id of the task that is awaiting on this op.
    #[prost(message, optional, tag = "4")]
    pub task_id: ::core::option::Option<super::common::Id>,
    /// Contains the operation poll stats.
    #[prost(message, optional, tag = "5")]
    pub poll_stats: ::core::option::Option<super::common::PollStats>,
    /// State attributes of the async op.
    #[prost(message, repeated, tag = "6")]
    pub attributes: ::prost::alloc::vec::Vec<super::common::Attribute>,
}
