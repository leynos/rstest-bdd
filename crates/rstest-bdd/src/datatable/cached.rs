//! Cached, shareable representations of data tables.
//!
//! `CachedTable` wraps an `Arc<Vec<Vec<String>>>`, enabling wrappers to
//! convert Gherkin data tables once and reuse the result across repeated step
//! executions without re-parsing the raw `&str` cells. Callers can either
//! borrow the rows for read-only access or clone them when ownership is
//! required.

use std::ops::Deref;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

#[cfg(any(test, feature = "diagnostics"))]
static CACHE_MISS_COUNT: AtomicUsize = AtomicUsize::new(0);

/// Record a cache miss for diagnostic or test visibility.
#[inline]
pub fn record_cache_miss() {
    #[cfg(any(test, feature = "diagnostics"))]
    {
        CACHE_MISS_COUNT.fetch_add(1, Ordering::Relaxed);
    }
}

/// Return the number of cache misses observed. Available in tests and when
/// the `diagnostics` feature is enabled.
#[cfg(any(test, feature = "diagnostics"))]
#[must_use]
pub fn cache_miss_count() -> usize {
    CACHE_MISS_COUNT.load(Ordering::Relaxed)
}

/// Reset the cache miss counter. Available in tests and when the `diagnostics`
/// feature is enabled.
#[cfg(any(test, feature = "diagnostics"))]
pub fn reset_cache_miss_count() {
    CACHE_MISS_COUNT.store(0, Ordering::Relaxed);
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
