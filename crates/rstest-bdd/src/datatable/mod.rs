//! Runtime helpers for working with typed Gherkin data tables.
mod cached;
mod error;
mod parsers;
mod rows;
mod spec;

pub use cached::{CachedTable, OwnedTableArc, record_cache_miss};
#[cfg(any(test, feature = "diagnostics"))]
pub use cached::{cache_miss_count, reset_cache_miss_count};
pub use error::DataTableError;
pub use parsers::{TrimmedParseError, TruthyBoolError, trimmed, truthy_bool};
pub use rows::{DataTableRow, Rows};
pub use spec::{HeaderSpec, RowSpec};

#[cfg(test)]
mod tests;
