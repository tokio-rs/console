use super::TaskId;
use std::{
    collections::HashMap,
    ops::{Deref, DerefMut},
};

#[derive(Default)]
pub(crate) struct TaskData<T> {
    data: HashMap<TaskId, (T, bool)>,
}

pub(crate) struct Updating<'a, T>(&'a mut (T, bool));

// === impl TaskData ===

impl<T> TaskData<T> {
    pub(crate) fn new() -> Self {
        Self {
            data: HashMap::new(),
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

    pub(crate) fn retain(&mut self, mut f: impl FnMut(&TaskId, &mut T, bool) -> bool) {
        self.data.retain(|id, (data, dirty)| f(id, data, *dirty))
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
