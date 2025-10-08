//! Error types surfaced by the datatable runtime.

use std::error::Error as StdError;

use thiserror::Error;

/// Errors that can arise when converting a Gherkin data table into typed rows.
///
/// `DataTableError` values surface through the generated step wrappers,
/// ensuring that failures are reported with helpful row and column context.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum DataTableError {
    /// Raised when a typed parser expects a header row but the table is empty.
    #[error("data table requires a header row")]
    MissingHeader,
    /// Raised when the header row repeats a column name.
    #[error("data table header contains duplicate column '{column}'")]
    DuplicateHeader { column: String },
    /// Raised when a row contains more or fewer cells than expected.
    #[error("data table row {row_number} has {actual} cells but expected {expected}")]
    UnevenRow {
        /// 1-based index of the row that failed, including any header.
        row_number: usize,
        /// Number of cells required for each row.
        expected: usize,
        /// Number of cells present in the offending row.
        actual: usize,
    },
    /// Raised when a column name lookup fails.
    #[error("data table row {row_number} is missing column '{column}'")]
    MissingColumn {
        /// 1-based index of the row that failed, including any header.
        row_number: usize,
        /// Name of the column that was requested.
        column: String,
    },
    /// Raised when positional access references an out-of-range column index.
    #[error("data table row {row_number} is missing cell {column_index}")]
    MissingCell {
        /// 1-based index of the row that failed, including any header.
        row_number: usize,
        /// 1-based index of the missing column.
        column_index: usize,
    },
    /// Raised when parsing an entire row fails.
    #[error("row {row_number}: {source}")]
    RowParse {
        /// 1-based index of the row that failed, including any header.
        row_number: usize,
        /// Root cause reported by the row parser.
        #[source]
        source: Box<dyn StdError + Send + Sync>,
    },
    /// Raised when parsing an individual cell fails.
    #[error("row {row_number}, column {column_index}{column_label}: {source}")]
    CellParse {
        /// 1-based index of the row that failed, including any header.
        row_number: usize,
        /// 1-based index of the column that failed to parse.
        column_index: usize,
        /// Display-formatted column label used when a header was present.
        column_label: String,
        /// Root cause reported by the cell parser.
        #[source]
        source: Box<dyn StdError + Send + Sync>,
    },
}

impl DataTableError {
    pub(crate) fn cell_parse<E>(
        row_number: usize,
        column_index: usize,
        column_label: Option<String>,
        err: E,
    ) -> Self
    where
        E: StdError + Send + Sync + 'static,
    {
        let column_label = column_label
            .map(|label| format!(" ({label})"))
            .unwrap_or_default();
        Self::CellParse {
            row_number,
            column_index: column_index + 1,
            column_label,
            source: Box::new(err),
        }
    }
}
