use super::TaskId;
use std::{
    collections::HashMap,
    mem,
    ops::{Deref, DerefMut},
};

pub(crate) struct TaskData<T> {
    data: HashMap<TaskId, (T, bool)>,
    shrink_every: usize,
    since_shrink: usize,
}

pub(crate) struct Updating<'a, T>(&'a mut (T, bool));

// === impl TaskData ===

impl<T> TaskData<T> {
    /// Shrinking every 60 flushes should be roughly every minute.
    pub(crate) const DEFAULT_SHRINK_INTERVAL: usize = 60;

    // APPROXIMATE memory used per entry. This is a lower-bound, not an accurate
    // size measurement, since the hashmap may use additional heap memory beyond
    // the size of a key + value pair.
    const APPROX_ENTRY_SZ: usize =
        mem::size_of::<T>() + mem::size_of::<TaskId>() + mem::size_of::<bool>();

    pub(crate) fn new() -> Self {
        Self {
            data: HashMap::new(),
            shrink_every: Self::DEFAULT_SHRINK_INTERVAL,
            since_shrink: 0,
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

    fn size_estimate_bytes(&self) -> usize {
        self.data.capacity() * Self::APPROX_ENTRY_SZ
    }

    pub(crate) fn retain(&mut self, mut f: impl FnMut(&TaskId, &mut T, bool) -> bool) -> bool {
        let len_0 = self.len();

        self.data.retain(|id, (data, dirty)| f(id, data, *dirty));

        let len_1 = self.len();

        // If no data was dropped on this pass, we're done!
        if len_1 == len_0 {
            tracing::trace!(
                len = len_0,
                size_estimate_bytes = self.size_estimate_bytes(),
                data = %std::any::type_name::<T>(),
                "no closed data was droppable",
            );
            return false;
        }

        // If we dropped some data, consider shrinking the hashmap.
        let should_shrink = self.since_shrink >= self.shrink_every;
        tracing::debug!(
            dropped = len_0 - len_1,
            len = len_1,
            should_shrink,
            since_shrink = self.since_shrink,
            size_estimate_bytes = self.size_estimate_bytes(),
            data = %std::any::type_name::<T>(),
            "dropped closed data",
        );

        if should_shrink {
            let size_0 = self.size_estimate_bytes();
            self.since_shrink = 0;
            self.data.shrink_to_fit();
            tracing::debug!(
                freed_bytes = size_0.saturating_sub(self.size_estimate_bytes()),
                size_estimate_bytes = self.size_estimate_bytes(),
                "shrank to fit"
            );
        } else {
            self.since_shrink += 1;
            tracing::trace!(self.since_shrink, size_estimate_bytes = %format_args!("{}B", self.size_estimate_bytes()));
        }

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
