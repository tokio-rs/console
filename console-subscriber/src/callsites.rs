use std::{
    fmt, ptr,
    sync::atomic::{AtomicPtr, AtomicUsize, Ordering},
};
use tracing_core::Metadata;

pub(crate) struct Callsites<const MAX_CALLSITES: usize> {
    ptrs: [AtomicPtr<Metadata<'static>>; MAX_CALLSITES],
    len: AtomicUsize,
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
        assert!(
            idx < MAX_CALLSITES,
            "you tried to store more than {} callsites, \
            time to make the callsite sets bigger i guess \
            (please open an issue for this)",
            MAX_CALLSITES,
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
            .finish()
    }
}
