// Some of these methods and re-exports may not be used currently.
#![allow(dead_code, unused_imports)]

#[cfg(feature = "parking_lot")]
pub(crate) use parking_lot_crate::{Mutex, MutexGuard, RwLock, RwLockReadGuard, RwLockWriteGuard};

#[cfg(not(feature = "parking_lot"))]
pub(crate) use self::std_impl::*;

#[cfg(not(feature = "parking_lot"))]
mod std_impl {
    use std::sync::{self, PoisonError, TryLockError};
    pub use std::sync::{MutexGuard, RwLockReadGuard, RwLockWriteGuard};

    #[derive(Debug, Default)]
    pub(crate) struct Mutex<T: ?Sized>(sync::Mutex<T>);

    impl<T> Mutex<T> {
        pub(crate) fn new(data: T) -> Self {
            Self(sync::Mutex::new(data))
        }
    }

    impl<T: ?Sized> Mutex<T> {
        pub(crate) fn read(&self) -> MutexGuard<'_, T> {
            self.0.lock().unwrap_or_else(PoisonError::into_inner)
        }
    }

    #[derive(Debug, Default)]
    pub(crate) struct RwLock<T: ?Sized>(sync::RwLock<T>);

    impl<T> RwLock<T> {
        pub(crate) fn new(data: T) -> Self {
            Self(sync::RwLock::new(data))
        }
    }

    impl<T: ?Sized> RwLock<T> {
        pub(crate) fn read(&self) -> RwLockReadGuard<'_, T> {
            self.0.read().unwrap_or_else(PoisonError::into_inner)
        }

        pub(crate) fn try_read(&self) -> Option<RwLockReadGuard<'_, T>> {
            match self.0.try_read() {
                Ok(guard) => Some(guard),
                Err(TryLockError::Poisoned(p)) => Some(p.into_inner()),
                Err(TryLockError::WouldBlock) => None,
            }
        }

        pub(crate) fn write(&self) -> RwLockWriteGuard<'_, T> {
            self.0.write().unwrap_or_else(PoisonError::into_inner)
        }

        pub(crate) fn try_write(&self) -> Option<RwLockWriteGuard<'_, T>> {
            match self.0.try_write() {
                Ok(guard) => Some(guard),
                Err(TryLockError::Poisoned(p)) => Some(p.into_inner()),
                Err(TryLockError::WouldBlock) => None,
            }
        }
    }
}
