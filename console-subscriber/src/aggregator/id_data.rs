use super::{shrink::ShrinkMap, DroppedAt, Id, Ids, ToProto};
use std::collections::{HashMap, HashSet};
use std::ops::{Deref, DerefMut};
use std::time::{Duration, SystemTime};

pub(crate) struct IdData<T> {
    data: ShrinkMap<Id, (T, bool)>,
}

pub(crate) struct Updating<'a, T>(&'a mut (T, bool));

pub(crate) enum Include {
    All,
    UpdatedOnly,
}

// === impl IdData ===

impl<T> Default for IdData<T> {
    fn default() -> Self {
        IdData {
            data: ShrinkMap::<Id, (T, bool)>::new(),
        }
    }
}

impl<T> IdData<T> {
    pub(crate) fn update_or_default(&mut self, id: Id) -> Updating<'_, T>
    where
        T: Default,
    {
        Updating(self.data.entry(id).or_default())
    }

    pub(crate) fn update(&mut self, id: &Id) -> Option<Updating<'_, T>> {
        self.data.get_mut(id).map(Updating)
    }

    pub(crate) fn insert(&mut self, id: Id, data: T) {
        self.data.insert(id, (data, true));
    }

    pub(crate) fn since_last_update(&mut self) -> impl Iterator<Item = (&Id, &mut T)> {
        self.data.iter_mut().filter_map(|(id, (data, dirty))| {
            if *dirty {
                *dirty = false;
                Some((id, data))
            } else {
                None
            }
        })
    }

    pub(crate) fn all(&self) -> impl Iterator<Item = (&Id, &T)> {
        self.data.iter().map(|(id, (data, _))| (id, data))
    }

    pub(crate) fn get(&self, id: &Id) -> Option<&T> {
        self.data.get(id).map(|(data, _)| data)
    }

    pub(crate) fn as_proto(&mut self, include: Include) -> HashMap<u64, T::Output>
    where
        T: ToProto,
    {
        match include {
            Include::UpdatedOnly => self
                .since_last_update()
                .map(|(id, d)| (*id, d.to_proto()))
                .collect(),
            Include::All => self.all().map(|(id, d)| (*id, d.to_proto())).collect(),
        }
    }

    pub(crate) fn drop_closed<R: DroppedAt>(
        &mut self,
        stats: &mut IdData<R>,
        now: SystemTime,
        retention: Duration,
        has_watchers: bool,
        ids: &mut Ids,
    ) {
        let _span = tracing::debug_span!(
            "drop_closed",
            entity = %std::any::type_name::<T>(),
            stats = %std::any::type_name::<R>(),
        )
        .entered();

        // drop closed entities
        tracing::trace!(?retention, has_watchers, "dropping closed");

        let mut dropped_ids = HashSet::new();
        stats.data.retain_and_shrink(|id, (stats, dirty)| {
            if let Some(dropped_at) = stats.dropped_at() {
                let dropped_for = now.duration_since(dropped_at).unwrap_or_default();
                let should_drop =
                        // if there are any clients watching, retain all dirty tasks regardless of age
                        (*dirty && has_watchers)
                        || dropped_for > retention;
                tracing::trace!(
                    stats.id = ?id,
                    stats.dropped_at = ?dropped_at,
                    stats.dropped_for = ?dropped_for,
                    stats.dirty = *dirty,
                    should_drop,
                );

                if should_drop {
                    dropped_ids.insert(*id);
                }
                return !should_drop;
            }

            true
        });

        // drop closed entities which no longer have stats.
        self.data
            .retain_and_shrink(|id, (_, _)| stats.data.contains_key(id));

        if !dropped_ids.is_empty() {
            // drop closed entities which no longer have stats.
            self.data
                .retain_and_shrink(|id, (_, _)| stats.data.contains_key(id));
            ids.remove_all(&dropped_ids);
        }
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
