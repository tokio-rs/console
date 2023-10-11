use crate::sync::RwLock;
use std::{
    collections::HashSet,
    fmt, ptr,
    sync::atomic::{AtomicPtr, AtomicUsize, Ordering},
};
use tracing_core::{callsite, Metadata};

pub(crate) struct Callsites<const MAX_CALLSITES: usize> {
    ptrs: [AtomicPtr<Metadata<'static>>; MAX_CALLSITES],
    len: AtomicUsize,
    spill: RwLock<HashSet<callsite::Identifier>>,
}

impl<const MAX_CALLSITES: usize> Callsites<MAX_CALLSITES> {
    #[track_caller]
    pub(crate) fn insert(&self, callsite: &'static Metadata<'static>) {
        // The callsite may already have been inserted, if the callsite cache
        // was invalidated and is being rebuilt. In that case, don't insert it
        // again.'
        if self.contains(callsite) {
            return;
        }

        let idx = self.len.fetch_add(1, Ordering::AcqRel);
        if idx < MAX_CALLSITES {
            // If there's still room in the callsites array, stick the address
            // in there.
            self.ptrs[idx]
                .compare_exchange(
                    ptr::null_mut(),
                    callsite as *const _ as *mut _,
                    Ordering::AcqRel,
                    Ordering::Acquire,
                )
                .expect("a callsite would have been clobbered by `insert` (this is a bug)");
        } else {
            // Otherwise, we've filled the callsite array (sad!). Spill over
            // into a hash set.
            self.spill.write().insert(callsite.callsite());
        }
    }

    pub(crate) fn contains(&self, callsite: &'static Metadata<'static>) -> bool {
        let mut start = 0;
        let mut len = self.len.load(Ordering::Acquire);
        loop {
            for cs in &self.ptrs[start..len] {
                let recorded = cs.load(Ordering::Acquire);
                if ptr::eq(recorded, callsite) {
                    return true;
                } else if ptr::eq(recorded, ptr::null_mut()) {
                    // We have read a recorded callsite before it has been
                    // written. We need to check again.
                    continue;
                }
            }

            // Did the length change while we were iterating over the callsite array?
            let new_len = self.len.load(Ordering::Acquire);
            if new_len > len {
                // If so, check again to see if the callsite is contained in any
                // callsites that were pushed since the last time we loaded `self.len`.
                start = len;
                len = new_len;
                continue;
            }

            // If the callsite array is not full, we have checked everything.
            if len <= MAX_CALLSITES {
                return false;
            }

            // Otherwise, we may have spilled over to the slower fallback hash
            // set. Check that.
            return self.check_spill(callsite);
        }
    }

    #[cold]
    fn check_spill(&self, callsite: &'static Metadata<'static>) -> bool {
        self.spill.read().contains(&callsite.callsite())
    }
}

impl<const MAX_CALLSITES: usize> Default for Callsites<MAX_CALLSITES> {
    fn default() -> Self {
        // It's necessary to use a `const` value here to initialize the array,
        // because `AtomicPtr` is not `Copy`.
        //
        // Clippy does not like when `const` values have interior mutability. See:
        // https://rust-lang.github.io/rust-clippy/master/index.html#declare_interior_mutable_const
        //
        // This is a warning because the const value is always copied when it's
        // used, so mutations to it will not be reflected in the `const` itself.
        // In some cases, this is a footgun (when you meant to use a `static`
        // item instead). However, in this case, that is *precisely* what we
        // want; the `const` value is being used as an initializer for the array
        // and it is *supposed* to be copied. Clippy's docs recommend ignoring
        // the lint when used as a legacy const initializer for a static item;
        // this is a very similar case.
        #[allow(clippy::declare_interior_mutable_const)]
        const NULLPTR: AtomicPtr<Metadata<'static>> = AtomicPtr::new(ptr::null_mut());
        Self {
            ptrs: [NULLPTR; MAX_CALLSITES],
            len: AtomicUsize::new(0),
            spill: Default::default(),
        }
    }
}

impl<const MAX_CALLSITES: usize> fmt::Debug for Callsites<MAX_CALLSITES> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let len = self.len.load(Ordering::Acquire);
        f.debug_struct("Callsites")
            .field("ptrs", &&self.ptrs[..len])
            .field("len", &len)
            .field("max_callsites", &MAX_CALLSITES)
            .field("spill", &self.spill)
            .finish()
    }
}
