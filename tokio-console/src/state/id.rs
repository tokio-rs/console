use std::{
    any, cmp,
    collections::hash_map::{Entry, HashMap},
    fmt,
    hash::{Hash, Hasher},
    marker::PhantomData,
};

pub(crate) struct Ids<T> {
    next: u64,
    map: HashMap<u64, Id<T>>,
}

/// A rewritten sequential ID.
///
/// This is distinct from the remote server's span ID, which may be reused and
/// is not sequential.
pub(crate) struct Id<T> {
    id: u64,
    _ty: PhantomData<fn(T)>,
}

// === impl Ids ===

impl<T> Ids<T> {
    pub(crate) fn id_for(&mut self, span_id: u64) -> Id<T> {
        match self.map.entry(span_id) {
            Entry::Occupied(entry) => *entry.get(),
            Entry::Vacant(entry) => {
                let id = Id {
                    id: self.next,
                    _ty: PhantomData,
                };
                entry.insert(id);
                self.next = self.next.wrapping_add(1);
                id
            }
        }
    }
}

impl<T> Default for Ids<T> {
    fn default() -> Self {
        Self {
            next: 1,
            map: Default::default(),
        }
    }
}

impl<T> fmt::Debug for Ids<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Ids")
            .field("next", &self.next)
            .field("map", &self.map)
            .field("type", &format_args!("{}", any::type_name::<T>()))
            .finish()
    }
}

// === impl Id ===

impl<T> Clone for Id<T> {
    #[inline]
    fn clone(&self) -> Self {
        Self {
            id: self.id,
            _ty: PhantomData,
        }
    }
}

impl<T> Copy for Id<T> {}

impl<T> fmt::Debug for Id<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let path = any::type_name::<T>();
        let type_name = path.split("::").last().unwrap_or(path);
        write!(f, "Id<{}>({})", type_name, self.id)
    }
}

impl<T> fmt::Display for Id<T> {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.id, f)
    }
}

impl<T> Hash for Id<T> {
    #[inline]
    fn hash<H: Hasher>(&self, state: &mut H) {
        state.write_u64(self.id);
    }
}

impl<T> PartialEq for Id<T> {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl<T> Eq for Id<T> {}

impl<T> cmp::Ord for Id<T> {
    #[inline]
    fn cmp(&self, other: &Self) -> cmp::Ordering {
        self.id.cmp(&other.id)
    }
}

impl<T> cmp::PartialOrd for Id<T> {
    #[inline]
    fn partial_cmp(&self, other: &Self) -> Option<cmp::Ordering> {
        Some(self.cmp(other))
    }
}
