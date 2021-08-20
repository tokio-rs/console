use super::TaskId;
use std::{
    any::type_name,
    collections::HashMap,
    hash::Hash,
    mem,
    ops::{Deref, DerefMut},
};

pub(crate) struct TaskData<T> {
    data: HashMap<TaskId, (T, bool)>,
    shrink: Shrink,
}

pub(crate) struct Updating<'a, T>(&'a mut (T, bool));

#[derive(Debug)]
pub(crate) struct Shrink {
    shrink_every: usize,
    since_shrink: usize,
    min_bytes: usize,
}

// === impl TaskData ===

impl<T> TaskData<T> {
    pub(crate) fn new() -> Self {
        Self {
            data: HashMap::new(),
            shrink: Shrink::default(),
        }
    }

    pub(crate) fn update_or_default(&mut self, id: TaskId) -> Updating<'_, T>
    where
        T: Default,
    {
        Updating(self.data.entry(id).or_default())
    }

    pub(crate) fn update(&mut self, id: &TaskId) -> Option<Updating<'_, T>> {
        self.data.get_mut(id).map(Updating)
    }

    pub(crate) fn insert(&mut self, id: TaskId, data: T) {
        self.data.insert(id, (data, true));
    }

    pub(crate) fn since_last_update(&mut self) -> impl Iterator<Item = (&TaskId, &mut T)> {
        self.data.iter_mut().filter_map(|(id, (data, dirty))| {
            if *dirty {
                *dirty = false;
                Some((id, data))
            } else {
                None
            }
        })
    }

    pub(crate) fn all(&self) -> impl Iterator<Item = (&TaskId, &T)> {
        self.data.iter().map(|(id, (data, _))| (id, data))
    }

    pub(crate) fn get(&self, id: &TaskId) -> Option<&T> {
        self.data.get(id).map(|(data, _)| data)
    }

    pub(crate) fn contains(&self, id: &TaskId) -> bool {
        self.data.contains_key(id)
    }

    pub(crate) fn len(&self) -> usize {
        self.data.len()
    }

    pub(crate) fn retain(&mut self, mut f: impl FnMut(&TaskId, &mut T, bool) -> bool) -> bool {
        let len_0 = self.len();

        self.data.retain(|id, (data, dirty)| f(id, data, *dirty));

        let len_1 = self.len();

        // If no data was dropped on this pass, we're done!
        if len_1 == len_0 {
            tracing::trace!(
                len = len_0,
                data = %type_name::<T>(),
                "no closed data was droppable",
            );
            return false;
        }

        // If we dropped some data, consider shrinking the hashmap.
        tracing::debug!(
            len = len_1,
            dropped = len_0.saturating_sub(len_1),
            data = %type_name::<T>(),
            "dropped closed data"
        );
        self.shrink.try_shrink_map(&mut self.data);

        true
    }
}

// === impl Updating ===

impl<'a, T> Deref for Updating<'a, T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        &self.0 .0
    }
}

impl<'a, T> DerefMut for Updating<'a, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0 .0
    }
}

impl<'a, T> Drop for Updating<'a, T> {
    fn drop(&mut self) {
        self.0 .1 = true;
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

    pub(crate) fn try_shrink_map<K, V>(&mut self, map: &mut HashMap<K, V>)
    where
        K: Hash + Eq,
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
