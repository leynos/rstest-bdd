//! Cached, shareable representations of data tables.
//!
//! `CachedTable` wraps an `Arc<Vec<Vec<String>>>`, enabling wrappers to
//! convert Gherkin data tables once and reuse the result across repeated step
//! executions without re-parsing the raw `&str` cells. Callers can either
//! borrow the rows for read-only access or clone them when ownership is
//! required.

use std::ops::Deref;
use std::sync::Arc;

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
