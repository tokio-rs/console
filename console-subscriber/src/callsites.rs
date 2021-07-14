use std::{
    ptr,
    sync::atomic::{AtomicPtr, AtomicUsize, Ordering},
};
use tracing_core::Metadata;

#[derive(Debug, Default)]
pub(crate) struct Callsites {
    ptrs: [AtomicPtr<Metadata<'static>>; MAX_CALLSITES],
    len: AtomicUsize,
}

// In practice each of these will have like, 1-5 callsites in it, max, so
// 32 is probably fine...if it ever becomes not fine, we'll fix that.
const MAX_CALLSITES: usize = 32;

impl Callsites {
    #[track_caller]
    pub(crate) fn insert(&self, callsite: &'static Metadata<'static>) {
        // The callsite may already have been inserted, if the callsite cache
        // was invalidated and is being rebuilt. In that case, don't insert it
        // again.'
        if self.contains(callsite) {
            return;
        }

        let idx = self.len.fetch_add(1, Ordering::AcqRel);
        assert!(
            idx < MAX_CALLSITES,
            "you tried to store more than 64 callsites, \
            time to make the callsite sets bigger i guess \
            (please open an issue for this)"
        );
        self.ptrs[idx]
            .compare_exchange(
                ptr::null_mut(),
                callsite as *const _ as *mut _,
                Ordering::AcqRel,
                Ordering::Acquire,
            )
            .expect("a callsite would have been clobbered by `insert` (this is a bug)");
    }

    pub(crate) fn contains(&self, callsite: &'static Metadata<'static>) -> bool {
        let len = self.len.load(Ordering::Acquire);
        for cs in &self.ptrs[..len] {
            if ptr::eq(cs.load(Ordering::Acquire), callsite) {
                return true;
            }
        }
        false
    }
}
