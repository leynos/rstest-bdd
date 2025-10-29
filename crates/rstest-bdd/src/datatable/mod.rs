//! Runtime helpers for working with typed Gherkin data tables.
mod error;
mod parsers;
mod rows;
mod spec;

pub use error::DataTableError;
pub use parsers::{trimmed, truthy_bool, TrimmedParseError, TruthyBoolError};
pub use rows::{DataTableRow, Rows};
pub use spec::{HeaderSpec, RowSpec};

#[cfg(test)]
mod tests;
