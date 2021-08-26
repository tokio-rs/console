use tracing_core::span::Id;

// This has been copied from tracing-subscriber. Once the library adds
// the ability to iterate over entered spans, this code will
// no longer be needed here
//
// https://github.com/tokio-rs/tracing/blob/master/tracing-subscriber/src/registry/stack.rs
#[derive(Debug, Clone)]
pub(crate) struct ContextId {
    id: Id,
    duplicate: bool,
}

impl ContextId {
    pub fn id(&self) -> &Id {
        &self.id
    }
}

/// `SpanStack` tracks what spans are currently executing on a thread-local basis.
///
/// A "separate current span" for each thread is a semantic choice, as each span
/// can be executing in a different thread.
#[derive(Debug, Default)]
pub(crate) struct SpanStack {
    stack: Vec<ContextId>,
}

impl SpanStack {
    #[inline]
    pub(crate) fn push(&mut self, id: Id) -> bool {
        let duplicate = self.stack.iter().any(|i| i.id == id);
        self.stack.push(ContextId { id, duplicate });
        !duplicate
    }

    #[inline]
    pub(crate) fn pop(&mut self, expected_id: &Id) -> bool {
        if let Some((idx, _)) = self
            .stack
            .iter()
            .enumerate()
            .rev()
            .find(|(_, ctx_id)| ctx_id.id == *expected_id)
        {
            let ContextId { id: _, duplicate } = self.stack.remove(idx);
            return !duplicate;
        }
        false
    }

    pub(crate) fn stack(&self) -> &Vec<ContextId> {
        &self.stack
    }
}
