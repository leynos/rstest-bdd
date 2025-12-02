//! Cached, shareable representations of data tables.
//!
//! `CachedTable` wraps an `Arc<Vec<Vec<String>>>`, enabling wrappers to
//! convert Gherkin data tables once and reuse the result across repeated step
//! executions without re-parsing the raw `&str` cells. Callers can either
//! borrow the rows for read-only access or clone them when ownership is
//! required.

use std::ops::Deref;
use std::sync::Arc;

#[cfg(any(test, feature = "diagnostics"))]
mod diagnostics {
    use std::collections::HashMap;
    use std::sync::{Mutex, OnceLock};
    use std::thread;

    fn counters() -> &'static Mutex<HashMap<thread::ThreadId, usize>> {
        static COUNTERS: OnceLock<Mutex<HashMap<thread::ThreadId, usize>>> = OnceLock::new();
        COUNTERS.get_or_init(|| Mutex::new(HashMap::new()))
    }

    pub(super) fn record_miss() {
        let mut map = counters()
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        *map.entry(thread::current().id()).or_insert(0) += 1;
    }

    pub(super) fn count() -> usize {
        counters()
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .get(&thread::current().id())
            .copied()
            .unwrap_or(0)
    }

    pub(super) fn reset() {
        counters()
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .remove(&thread::current().id());
    }
}

/// Record a cache miss for diagnostic or test visibility.
#[inline]
pub fn record_cache_miss() {
    #[cfg(any(test, feature = "diagnostics"))]
    diagnostics::record_miss();
}

/// Return the number of cache misses observed by the current thread. Available
/// in tests and when the `diagnostics` feature is enabled.
#[cfg(any(test, feature = "diagnostics"))]
#[must_use]
pub fn cache_miss_count() -> usize {
    diagnostics::count()
}

/// Reset the cache miss counter for the current thread. Available in tests and
/// when the `diagnostics` feature is enabled.
#[cfg(any(test, feature = "diagnostics"))]
pub fn reset_cache_miss_count() {
    diagnostics::reset();
}

/// Shareable view of a parsed data table.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CachedTable {
    rows: Arc<Vec<Vec<String>>>,
}

impl CachedTable {
    /// Construct a cache from owned rows.
    #[must_use]
    pub fn new(rows: Vec<Vec<String>>) -> Self {
        Self {
            rows: Arc::new(rows),
        }
    }

    /// Construct a cache from an existing shared table.
    #[must_use]
    pub fn from_arc(rows: Arc<Vec<Vec<String>>>) -> Self {
        Self { rows }
    }

    /// Borrow the cached rows.
    #[must_use]
    pub fn as_rows(&self) -> &[Vec<String>] {
        self.rows.as_slice()
    }

    /// Borrow the underlying shared allocation without cloning the `Arc`.
    #[must_use]
    pub fn as_arc_ref(&self) -> &Arc<Vec<Vec<String>>> {
        &self.rows
    }

    /// Obtain a stable pointer to the shared allocation.
    #[must_use]
    pub fn as_ptr(&self) -> *const Vec<Vec<String>> {
        Arc::as_ptr(&self.rows)
    }

    /// Access the underlying shared allocation.
    #[must_use]
    pub fn as_arc(&self) -> Arc<Vec<Vec<String>>> {
        Arc::clone(&self.rows)
    }
}

impl Deref for CachedTable {
    type Target = [Vec<String>];

    fn deref(&self) -> &Self::Target {
        self.as_rows()
    }
}

impl AsRef<[Vec<String>]> for CachedTable {
    fn as_ref(&self) -> &[Vec<String>] {
        self.as_rows()
    }
}

impl From<Vec<Vec<String>>> for CachedTable {
    fn from(rows: Vec<Vec<String>>) -> Self {
        Self::new(rows)
    }
}

impl From<CachedTable> for Vec<Vec<String>> {
    fn from(value: CachedTable) -> Self {
        (*value.rows).clone()
    }
}
