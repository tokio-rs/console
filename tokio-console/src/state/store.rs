use std::{
    any,
    cell::{self, RefCell},
    cmp,
    collections::hash_map::{self, Entry, HashMap},
    fmt,
    hash::{Hash, Hasher},
    marker::PhantomData,
    rc::{Rc, Weak},
    vec,
};

use super::Visibility;

/// Stores a set of items which are associated with a [`SpanId`] and a rewritten
/// sequential [`Id`].
#[derive(Debug)]
pub(crate) struct Store<T> {
    ids: Ids<T>,
    store: HashMap<Id<T>, Stored<T>>,
    new_items: Vec<Ref<T>>,
}

pub(crate) type Ref<T> = Weak<RefCell<T>>;
pub(crate) type Stored<T> = Rc<RefCell<T>>;
pub(crate) type SpanId = u64;

/// A rewritten sequential ID.
///
/// This is distinct from the remote server's span ID, which may be reused and
/// is not sequential.
pub(crate) struct Id<T> {
    id: u64,
    _ty: PhantomData<fn(T)>,
}

/// Stores the rewritten sequential IDs of items in a [`Store`].
pub(crate) struct Ids<T> {
    next: u64,
    map: HashMap<u64, Id<T>>,
}

// === impl Store ===

impl<T> Store<T> {
    pub fn get(&self, id: Id<T>) -> Option<&Stored<T>> {
        self.store.get(&id)
    }

    pub fn get_by_span(&self, span_id: SpanId) -> Option<&Stored<T>> {
        let id = self.ids.map.get(&span_id)?;
        self.get(*id)
    }

    pub fn ids_mut(&mut self) -> &mut Ids<T> {
        &mut self.ids
    }

    /// Given an iterator of `U`-typed items and a function `f` mapping a
    /// `U`-typed item to a `T`-typed item and an [`Id`] for that item, inserts
    /// the `T`-typed items into the store along with their IDs.
    ///
    /// This function has an admittedly somewhat complex signature. It would be
    /// nicer if this could just be an `iter::Extend` implementation, but that
    /// makes borrowing the set of [`Ids`] in the closure that's mapped over the
    /// iterator challenging, because the `extend` method mutably borrows the
    /// whole `Store`.
    pub fn insert_with<U>(
        &mut self,
        visibility: Visibility,
        items: impl IntoIterator<Item = U>,
        mut f: impl FnMut(&mut Ids<T>, U) -> Option<(Id<T>, T)>,
    ) {
        self.set_visibility(visibility);
        let items = items
            .into_iter()
            .filter_map(|item| f(&mut self.ids, item))
            .map(|(id, item)| {
                let item = Rc::new(RefCell::new(item));
                self.new_items.push(Rc::downgrade(&item));
                (id, item)
            });
        self.store.extend(items);
    }

    pub fn updated<'store, U, I>(
        &'store mut self,
        update: I,
    ) -> impl Iterator<Item = (U, cell::RefMut<'store, T>)> + 'store
    where
        I: IntoIterator<Item = (SpanId, U)>,
        I::IntoIter: 'store,
    {
        update.into_iter().filter_map(|(span_id, update)| {
            let id = self.ids.map.get(&span_id)?;
            let item = self.store.get(id)?;
            Some((update, item.borrow_mut()))
        })
    }

    /// Applies a predicate to each element in the [`Store`], removing the item
    /// if the predicate returns `false`.
    pub fn retain(&mut self, f: impl FnMut(&Id<T>, &mut Stored<T>) -> bool) {
        self.store.retain(f);
        // If a removed element was in `new_items`, remove it.
        self.new_items.retain(|item| item.upgrade().is_some());
        // TODO(eliza): remove from `ids` if it's no longer in `store`?
    }

    /// Returns an iterator over all of the items which have been added to this
    /// `Store` since the last time `take_new_items` was called.
    pub fn take_new_items(&mut self) -> vec::Drain<'_, Ref<T>> {
        self.new_items.drain(..)
    }

    pub fn values(&self) -> hash_map::Values<'_, Id<T>, Stored<T>> {
        self.store.values()
    }

    pub fn iter(&self) -> hash_map::Iter<'_, Id<T>, Stored<T>> {
        self.store.iter()
    }

    fn set_visibility(&mut self, visibility: Visibility) {
        if matches!(visibility, Visibility::Show) {
            self.new_items.clear();
        }
    }
}

impl<T> Default for Store<T> {
    fn default() -> Self {
        Self {
            ids: Ids::default(),
            store: HashMap::default(),
            new_items: Vec::default(),
        }
    }
}

impl<'store, T> IntoIterator for &'store Store<T> {
    type Item = (&'store Id<T>, &'store Stored<T>);
    type IntoIter = hash_map::Iter<'store, Id<T>, Stored<T>>;

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

// === impl Ids ===

impl<T> Ids<T> {
    pub(crate) fn id_for(&mut self, span_id: SpanId) -> Id<T> {
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
