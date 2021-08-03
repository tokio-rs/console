tonic::include_proto!("rs.tokio.console.tasks");

// === IDs ===

impl From<u64> for TaskId {
    fn from(id: u64) -> Self {
        TaskId { id }
    }
}

impl From<TaskId> for u64 {
    fn from(id: TaskId) -> Self {
        id.id
    }
}

impl Copy for TaskId {}
