use super::{shrink::ShrinkMap, Id, ToProto};
use crate::stats::{DroppedAt, Unsent};
use std::collections::HashMap;
use std::time::{Duration, SystemTime};

pub(crate) struct IdData<T> {
    data: ShrinkMap<Id, T>,
}

pub(crate) enum Include {
    All,
    UpdatedOnly,
}

// === impl IdData ===

impl<T> Default for IdData<T> {
    fn default() -> Self {
        IdData {
            data: ShrinkMap::<Id, T>::new(),
        }
    }
}

impl<T: Unsent> IdData<T> {
    pub(crate) fn insert(&mut self, id: Id, data: T) {
        self.data.insert(id, data);
    }

    pub(crate) fn since_last_update(&mut self) -> impl Iterator<Item = (&Id, &mut T)> {
        self.data.iter_mut().filter_map(|(id, data)| {
            if data.take_unsent() {
                Some((id, data))
            } else {
                None
            }
        })
    }

    pub(crate) fn all(&self) -> impl Iterator<Item = (&Id, &T)> {
        self.data.iter()
    }

    pub(crate) fn get(&self, id: &Id) -> Option<&T> {
        self.data.get(id)
    }

    pub(crate) fn as_proto(&mut self, include: Include) -> HashMap<u64, T::Output>
    where
        T: ToProto,
    {
        match include {
            Include::UpdatedOnly => self
                .since_last_update()
                .map(|(id, d)| (id.into_u64(), d.to_proto()))
                .collect(),
            Include::All => self
                .all()
                .map(|(id, d)| (id.into_u64(), d.to_proto()))
                .collect(),
        }
    }

    pub(crate) fn drop_closed<R: DroppedAt + Unsent>(
        &mut self,
        stats: &mut IdData<R>,
        now: SystemTime,
        retention: Duration,
        has_watchers: bool,
    ) {
        let _span = tracing::debug_span!(
            "drop_closed",
            entity = %std::any::type_name::<T>(),
            stats = %std::any::type_name::<R>(),
        )
        .entered();

        // drop closed entities
        tracing::trace!(?retention, has_watchers, "dropping closed");

        stats.data.retain_and_shrink(|id, stats| {
            if let Some(dropped_at) = stats.dropped_at() {
                let dropped_for = now.duration_since(dropped_at).unwrap_or_default();
                let dirty = stats.is_unsent();
                let should_drop =
                        // if there are any clients watching, retain all dirty tasks regardless of age
                        (dirty && has_watchers)
                        || dropped_for > retention;
                tracing::trace!(
                    stats.id = ?id,
                    stats.dropped_at = ?dropped_at,
                    stats.dropped_for = ?dropped_for,
                    stats.dirty = dirty,
                    should_drop,
                );
                return !should_drop;
            }

            true
        });

        // drop closed entities which no longer have stats.
        self.data
            .retain_and_shrink(|id, _| stats.data.contains_key(id));
    }
}
