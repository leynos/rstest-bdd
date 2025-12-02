//! Runtime helpers for working with typed Gherkin data tables.
mod cached;
mod error;
mod parsers;
mod rows;
mod spec;

#[cfg(any(test, feature = "diagnostics"))]
pub use cached::{cache_miss_count, reset_cache_miss_count};
pub use cached::{record_cache_miss, CachedTable};
pub use error::DataTableError;
pub use parsers::{trimmed, truthy_bool, TrimmedParseError, TruthyBoolError};
pub use rows::{DataTableRow, Rows};
pub use spec::{HeaderSpec, RowSpec};

#[cfg(test)]
mod tests;
