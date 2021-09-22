use std::{
    borrow::{Borrow, Cow},
    collections::HashSet,
    fmt,
    hash::Hash,
    ops::Deref,
    rc::Rc,
};

/// A (painfully simple) string interner.
///
/// A nicer implementation is almost certainly possible. However, this one is
/// simple and doesn't involve any unsafe code. We could almost certainly
/// replace it with something faster if it becomes a bottleneck.
#[derive(Debug, Default)]
pub(crate) struct Strings {
    strings: HashSet<InternedStr>,
}

#[derive(Clone, Hash, Eq, PartialEq, Ord, PartialOrd)]
pub(crate) struct InternedStr(Rc<String>);

impl Strings {
    // NOTE(elzia): currently, we never need to use this, but we can always
    // uncomment it if we do...

    // pub(crate) fn string_ref<Q>(&mut self, string: &Q) -> InternedStr
    // where
    //     InternedStr: Borrow<Q>,
    //     Q: Hash + Eq + ToOwned<Owned = String>,
    // {
    //     if let Some(s) = self.strings.get(string) {
    //         return s.clone();
    //     }

    //     self.insert(string.to_owned())
    // }

    pub(crate) fn string(&mut self, string: String) -> InternedStr {
        if let Some(s) = self.strings.get(&string) {
            return s.clone();
        }

        self.insert(string)
    }

    fn insert(&mut self, string: String) -> InternedStr {
        let string = InternedStr(Rc::new(string));
        self.strings.insert(string.clone());
        string
    }
}

// === impl InternedStr ===

impl Deref for InternedStr {
    type Target = str;

    #[inline]
    fn deref(&self) -> &Self::Target {
        self.0.deref()
    }
}

impl AsRef<str> for InternedStr {
    #[inline]
    fn as_ref(&self) -> &str {
        self.0.as_ref()
    }
}

impl fmt::Display for InternedStr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self.0.deref(), f)
    }
}

impl Borrow<str> for InternedStr {
    fn borrow(&self) -> &str {
        self.0.deref()
    }
}

impl Borrow<String> for InternedStr {
    fn borrow(&self) -> &String {
        self.0.deref()
    }
}

impl<'a> From<&'a InternedStr> for Cow<'a, str> {
    fn from(istr: &'a InternedStr) -> Self {
        Cow::Borrowed(istr)
    }
}

impl fmt::Debug for InternedStr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let include_refs = f.alternate();
        let mut tuple = f.debug_tuple("InternedStr");
        tuple.field(&self.0);
        if include_refs {
            tuple.field(&Rc::strong_count(&self.0));
        }
        tuple.finish()
    }
}
