//! Poison-recovering synchronization helpers shared across the workspace.
//!
//! These helpers retain access to synchronization state after a panic while a
//! lock was held. Callers that must report poisoning to their own callers use
//! the standard-library lock APIs directly instead.

use std::sync::{
    LockResult,
    Mutex,
    MutexGuard,
    PoisonError,
    RwLock,
    RwLockReadGuard,
    RwLockWriteGuard,
};

/// Extract a lock result's inner value, retaining poisoned state for callers.
fn recover_lock_result<T>(result: LockResult<T>) -> T {
    result.unwrap_or_else(PoisonError::into_inner)
}

/// Acquire a mutex guard while recovering from poison rather than panicking.
///
/// Do not use this method when the caller must propagate poison as an error.
///
/// # Examples
/// ```
/// use std::sync::Mutex;
///
/// use rstest_bdd_patterns::MutexExt;
///
/// let value = Mutex::new(42);
/// assert_eq!(*value.lock_ignoring_poison(), 42);
/// ```
pub trait MutexExt<T> {
    /// Acquire this mutex's guard while recovering from poison rather than panicking.
    ///
    /// Do not use this method when the caller must propagate poison as an error.
    ///
    /// # Examples
    /// ```
    /// use std::sync::Mutex;
    ///
    /// use rstest_bdd_patterns::MutexExt;
    ///
    /// let value = Mutex::new(42);
    /// assert_eq!(*value.lock_ignoring_poison(), 42);
    /// ```
    #[must_use = "the returned guard must remain held while accessing the protected value"]
    fn lock_ignoring_poison(&self) -> MutexGuard<'_, T>;
}

impl<T> MutexExt<T> for Mutex<T> {
    fn lock_ignoring_poison(&self) -> MutexGuard<'_, T> { recover_lock_result(self.lock()) }
}

/// Acquire read and write guards while recovering from poison rather than panicking.
///
/// Do not use these methods when the caller must propagate poison as an error.
///
/// # Examples
/// ```
/// use std::sync::RwLock;
///
/// use rstest_bdd_patterns::RwLockExt;
///
/// let value = RwLock::new(42);
/// assert_eq!(*value.read_ignoring_poison(), 42);
/// *value.write_ignoring_poison() = 43;
/// assert_eq!(*value.read_ignoring_poison(), 43);
/// ```
pub trait RwLockExt<T> {
    /// Acquire this lock's read guard while recovering from poison rather than panicking.
    ///
    /// Do not use this method when the caller must propagate poison as an error.
    ///
    /// # Examples
    /// ```
    /// use std::sync::RwLock;
    ///
    /// use rstest_bdd_patterns::RwLockExt;
    ///
    /// let value = RwLock::new(42);
    /// assert_eq!(*value.read_ignoring_poison(), 42);
    /// ```
    #[must_use = "the returned guard must remain held while accessing the protected value"]
    fn read_ignoring_poison(&self) -> RwLockReadGuard<'_, T>;

    /// Acquire this lock's write guard while recovering from poison rather than panicking.
    ///
    /// Do not use this method when the caller must propagate poison as an error.
    ///
    /// # Examples
    /// ```
    /// use std::sync::RwLock;
    ///
    /// use rstest_bdd_patterns::RwLockExt;
    ///
    /// let value = RwLock::new(42);
    /// *value.write_ignoring_poison() = 43;
    /// assert_eq!(*value.read_ignoring_poison(), 43);
    /// ```
    #[must_use = "the returned guard must remain held while accessing the protected value"]
    fn write_ignoring_poison(&self) -> RwLockWriteGuard<'_, T>;
}

impl<T> RwLockExt<T> for RwLock<T> {
    fn read_ignoring_poison(&self) -> RwLockReadGuard<'_, T> { recover_lock_result(self.read()) }

    fn write_ignoring_poison(&self) -> RwLockWriteGuard<'_, T> { recover_lock_result(self.write()) }
}

/// Recover an owned lock value from poison rather than panicking.
///
/// This function accepts the `LockResult` returned by owned extraction such as
/// [`Mutex::into_inner`]. Do not use it when the caller must propagate poison
/// as an error.
///
/// # Examples
/// ```
/// use std::sync::Mutex;
///
/// use rstest_bdd_patterns::recover_poison;
///
/// assert_eq!(recover_poison(Mutex::new(42).into_inner()), 42);
/// ```
#[must_use]
pub fn recover_poison<T>(result: LockResult<T>) -> T { recover_lock_result(result) }

#[cfg(test)]
mod tests {
    //! Tests pinning recovery behaviour for poisoned mutexes and read-write locks.

    use std::{
        sync::{Arc, Mutex, RwLock},
        thread,
    };

    use super::{MutexExt, RwLockExt};

    #[test]
    fn recovers_poisoned_mutex_value() {
        let value = Arc::new(Mutex::new(42));
        let poisoned = Arc::clone(&value);
        let _ = thread::spawn(move || {
            let _guard = poisoned
                .lock()
                .expect("test mutex should lock before poisoning");
            panic!("poison test mutex");
        })
        .join();

        assert!(value.is_poisoned());
        assert_eq!(*value.lock_ignoring_poison(), 42);
    }

    #[test]
    fn recovers_poisoned_read_write_lock_value() {
        let value = Arc::new(RwLock::new(42));
        let poisoned = Arc::clone(&value);
        let _ = thread::spawn(move || {
            let _guard = poisoned
                .write()
                .expect("test read-write lock should lock before poisoning");
            panic!("poison test read-write lock");
        })
        .join();

        assert!(value.is_poisoned());
        assert_eq!(*value.read_ignoring_poison(), 42);
        *value.write_ignoring_poison() = 43;
        assert_eq!(*value.read_ignoring_poison(), 43);
    }
}
