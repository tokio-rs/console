use std::{
    any::type_name,
    collections::hash_map::{HashMap, RandomState},
    hash::{BuildHasher, Hash},
    mem,
    ops::{Deref, DerefMut},
};

#[derive(Debug, Clone)]
pub(crate) struct ShrinkMap<K, V, S = RandomState> {
    map: HashMap<K, V, S>,
    shrink: Shrink,
}

#[derive(Debug, Clone)]
pub(crate) struct ShrinkVec<T> {
    vec: Vec<T>,
    shrink: Shrink,
}

#[derive(Debug, Clone)]
pub(crate) struct Shrink {
    shrink_every: usize,
    since_shrink: usize,
    min_bytes: usize,
}

// === impl ShrinkMap ===

impl<K, V> ShrinkMap<K, V>
where
    K: Hash + Eq,
{
    pub(crate) fn new() -> Self {
        Self {
            map: HashMap::new(),
            shrink: Shrink::default(),
        }
    }
}

impl<K, V, S> ShrinkMap<K, V, S>
where
    K: Hash + Eq,
    S: BuildHasher,
{
    pub(crate) fn try_shrink(&mut self) {
        self.shrink.try_shrink_map(&mut self.map)
    }

    pub(crate) fn retain_and_shrink(&mut self, f: impl FnMut(&K, &mut V) -> bool) {
        let len0 = self.len();

        self.retain(f);

        if self.len() < len0 {
            tracing::debug!(
                len = self.len(),
                dropped = len0.saturating_sub(self.len()),
                data.key = %type_name::<K>(),
                data.val = %type_name::<V>(),
                "dropped unused entries"
            );
            self.try_shrink();
        }
    }
}

impl<K, V, S> Deref for ShrinkMap<K, V, S> {
    type Target = HashMap<K, V, S>;
    fn deref(&self) -> &Self::Target {
        &self.map
    }
}

impl<K, V, S> DerefMut for ShrinkMap<K, V, S> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.map
    }
}

impl<K, V> Default for ShrinkMap<K, V>
where
    K: Hash + Eq,
{
    fn default() -> Self {
        Self::new()
    }
}

// === impl ShrinkVec ===

impl<T> ShrinkVec<T> {
    pub(crate) fn new() -> Self {
        Self {
            vec: Vec::new(),
            shrink: Shrink::default(),
        }
    }

    pub(crate) fn try_shrink(&mut self) {
        self.shrink.try_shrink_vec(&mut self.vec)
    }

    pub(crate) fn retain_and_shrink(&mut self, f: impl FnMut(&T) -> bool) {
        let len0 = self.len();

        self.retain(f);

        if self.len() < len0 {
            tracing::debug!(
                len = self.len(),
                dropped = len0.saturating_sub(self.len()),
                data = %type_name::<T>(),
                "dropped unused data"
            );
            self.try_shrink();
        }
    }
}

impl<T> Deref for ShrinkVec<T> {
    type Target = Vec<T>;
    fn deref(&self) -> &Self::Target {
        &self.vec
    }
}

impl<T> DerefMut for ShrinkVec<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.vec
    }
}

impl<T> Default for ShrinkVec<T> {
    fn default() -> Self {
        Self::new()
    }
}

// === impl Shrink ===

impl Shrink {
    /// Shrinking every 60 flushes should be roughly every minute.
    pub(crate) const DEFAULT_SHRINK_INTERVAL: usize = 60;

    /// Don't bother if we'd free less than 4KB of memory.
    // TODO(eliza): this number was chosen totally arbitrarily; it's the minimum
    // page size on x86.
    pub(crate) const DEFAULT_MIN_SIZE_BYTES: usize = 1024 * 4;

    pub(crate) fn try_shrink_map<K, V, S>(&mut self, map: &mut HashMap<K, V, S>)
    where
        K: Hash + Eq,
        S: BuildHasher,
    {
        if self.should_shrink::<(K, V)>(map.capacity(), map.len()) {
            map.shrink_to_fit();
        }
    }

    pub(crate) fn try_shrink_vec<T>(&mut self, vec: &mut Vec<T>) {
        if self.should_shrink::<T>(vec.capacity(), vec.len()) {
            vec.shrink_to_fit();
        }
    }

    /// Returns `true` if we should shrink with a capacity of `capacity` Ts and
    /// an actual length of `len` Ts.
    fn should_shrink<T>(&mut self, capacity: usize, len: usize) -> bool {
        // Has the required interval elapsed since the last shrink?
        self.since_shrink = self.since_shrink.saturating_add(1);
        if self.since_shrink < self.shrink_every {
            tracing::trace!(
                self.since_shrink,
                self.shrink_every,
                capacity_bytes = capacity * mem::size_of::<T>(),
                used_bytes = len * mem::size_of::<T>(),
                data = %type_name::<T>(),
                "should_shrink: shrink interval has not elapsed"
            );
            return false;
        }

        // Okay, would we free up at least `min_bytes` by shrinking?
        let capacity_bytes = capacity * mem::size_of::<T>();
        let used_bytes = len * mem::size_of::<T>();
        let diff = capacity_bytes.saturating_sub(used_bytes);
        if diff < self.min_bytes {
            tracing::trace!(
                self.since_shrink,
                self.shrink_every,
                self.min_bytes,
                freed_bytes = diff,
                capacity_bytes,
                used_bytes,
                data = %type_name::<T>(),
                "should_shrink: would not free sufficient bytes"
            );
            return false;
        }

        // Reset the clock! time to shrink!
        self.since_shrink = 0;
        tracing::debug!(
            self.since_shrink,
            self.shrink_every,
            self.min_bytes,
            freed_bytes = diff,
            capacity_bytes,
            used_bytes,
            data = %type_name::<T>(),
            "should_shrink: shrinking!"
        );
        true
    }
}

impl Default for Shrink {
    fn default() -> Self {
        Self {
            since_shrink: 0,
            shrink_every: Self::DEFAULT_SHRINK_INTERVAL,
            min_bytes: Self::DEFAULT_MIN_SIZE_BYTES,
        }
    }
}
