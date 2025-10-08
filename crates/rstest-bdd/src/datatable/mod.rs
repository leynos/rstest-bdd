//! Runtime helpers for working with typed Gherkin data tables.
mod error;
mod parsers;
mod rows;
mod spec;

pub use error::DataTableError;
pub use parsers::{TrimmedParseError, TruthyBoolError, trimmed, truthy_bool};
pub use rows::{DataTableRow, Rows};
pub use spec::{HeaderSpec, RowSpec};

#[cfg(test)]
mod tests;
