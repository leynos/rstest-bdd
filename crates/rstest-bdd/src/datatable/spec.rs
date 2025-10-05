use std::collections::HashMap;
use std::error::Error as StdError;

use super::DataTableError;

/// Metadata describing the header row of a data table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HeaderSpec {
    columns: Vec<String>,
    index: HashMap<String, usize>,
}

impl HeaderSpec {
    /// Builds a [`HeaderSpec`] from the supplied header row.
    ///
    /// # Errors
    ///
    /// Returns [`DataTableError::DuplicateHeader`] when column names repeat.
    pub fn new(header_row: Vec<String>) -> Result<Self, DataTableError> {
        let mut index = HashMap::with_capacity(header_row.len());
        for (idx, column) in header_row.iter().enumerate() {
            if index.insert(column.clone(), idx).is_some() {
                return Err(DataTableError::DuplicateHeader {
                    column: column.clone(),
                });
            }
        }
        Ok(Self {
            columns: header_row,
            index,
        })
    }

    /// Returns the number of columns declared in the header.
    #[must_use]
    pub fn len(&self) -> usize {
        self.columns.len()
    }

    /// Returns `true` when the header is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.columns.is_empty()
    }

    /// Fetches a column name by index.
    #[must_use]
    pub fn column(&self, index: usize) -> Option<&str> {
        self.columns.get(index).map(String::as_str)
    }

    /// Looks up the index for a column, returning an error when missing.
    ///
    /// # Errors
    ///
    /// Returns [`DataTableError::MissingColumn`] when the column is absent.
    pub fn require(&self, name: &str, row_number: usize) -> Result<usize, DataTableError> {
        self.index
            .get(name)
            .copied()
            .ok_or_else(|| DataTableError::MissingColumn {
                row_number,
                column: name.to_string(),
            })
    }

    /// Returns the names of all columns.
    #[must_use]
    pub fn columns(&self) -> &[String] {
        &self.columns
    }
}

/// Representation of a single row within a data table.
#[derive(Debug)]
pub struct RowSpec<'h> {
    header: Option<&'h HeaderSpec>,
    row_number: usize,
    index: usize,
    cells: Vec<String>,
    indices: Vec<Option<usize>>,
}

impl<'h> RowSpec<'h> {
    pub(super) fn new(
        header: Option<&'h HeaderSpec>,
        row_number: usize,
        index: usize,
        cells: Vec<String>,
    ) -> Self {
        let indices = (0..cells.len()).map(Some).collect();
        Self {
            header,
            row_number,
            index,
            cells,
            indices,
        }
    }

    /// Returns the 1-based row number, including any header.
    #[must_use]
    pub fn row_number(&self) -> usize {
        self.row_number
    }

    /// Returns the zero-based row index relative to the data rows (excluding
    /// the header).
    #[must_use]
    pub fn index(&self) -> usize {
        self.index
    }

    /// Returns the number of cells in the row.
    #[must_use]
    pub fn len(&self) -> usize {
        self.cells.len()
    }

    /// Returns `true` when the row contains no cells.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.cells.is_empty()
    }

    /// Provides immutable access to the underlying cells.
    #[must_use]
    pub fn cells(&self) -> &[String] {
        &self.cells
    }

    /// Retrieves the cell at a given index.
    ///
    /// # Errors
    ///
    /// Returns [`DataTableError::MissingCell`] when the index is out of range.
    pub fn cell(&self, column_index: usize) -> Result<&str, DataTableError> {
        let Some(position) = self.indices.get(column_index).and_then(|p| *p) else {
            return Err(DataTableError::MissingCell {
                row_number: self.row_number,
                column_index,
            });
        };
        self.cells
            .get(position)
            .map(String::as_str)
            .ok_or(DataTableError::MissingCell {
                row_number: self.row_number,
                column_index,
            })
    }

    /// Removes and returns the cell at `column_index`.
    ///
    /// # Errors
    ///
    /// Returns [`DataTableError::MissingCell`] when the index is out of range.
    pub fn take_cell(&mut self, column_index: usize) -> Result<String, DataTableError> {
        let Some(position) = self.indices.get(column_index).and_then(|p| *p) else {
            return Err(DataTableError::MissingCell {
                row_number: self.row_number,
                column_index,
            });
        };
        let value = self.cells.remove(position);
        if let Some(slot) = self.indices.get_mut(column_index) {
            *slot = None;
        }
        for slot in self.indices.iter_mut().flatten() {
            if *slot > position {
                *slot -= 1;
            }
        }
        Ok(value)
    }

    /// Retrieves the cell associated with a header column.
    ///
    /// # Errors
    ///
    /// Returns [`DataTableError::MissingHeader`] or
    /// [`DataTableError::MissingColumn`] when the lookup fails.
    pub fn column(&self, name: &str) -> Result<&str, DataTableError> {
        let header = self.header.ok_or(DataTableError::MissingHeader)?;
        let index = header.require(name, self.row_number)?;
        self.cell(index)
    }

    /// Removes and returns the cell associated with a header column.
    ///
    /// # Errors
    ///
    /// Returns [`DataTableError::MissingHeader`] or
    /// [`DataTableError::MissingColumn`] when the lookup fails.
    pub fn take_column(&mut self, name: &str) -> Result<String, DataTableError> {
        let header = self.header.ok_or(DataTableError::MissingHeader)?;
        let index = header.require(name, self.row_number)?;
        self.take_cell(index)
    }

    /// Parses a cell using a user-supplied parser.
    ///
    /// # Errors
    ///
    /// Returns [`DataTableError::MissingCell`] when the index is out of range
    /// or [`DataTableError::CellParse`] when the parser fails.
    pub fn parse_with<F, T, E>(&self, column_index: usize, parser: F) -> Result<T, DataTableError>
    where
        F: FnOnce(&str) -> Result<T, E>,
        E: StdError + Send + Sync + 'static,
    {
        let value = self.cell(column_index)?;
        parser(value).map_err(|err| {
            let label = self
                .header
                .and_then(|h| h.column(column_index))
                .map(ToOwned::to_owned);
            DataTableError::cell_parse(self.row_number, column_index, label, err)
        })
    }

    /// Parses a named column using a user-supplied parser.
    ///
    /// # Errors
    ///
    /// Returns [`DataTableError::MissingHeader`],
    /// [`DataTableError::MissingColumn`], or
    /// [`DataTableError::CellParse`] when access or parsing fails.
    pub fn parse_column_with<F, T, E>(&self, name: &str, parser: F) -> Result<T, DataTableError>
    where
        F: FnOnce(&str) -> Result<T, E>,
        E: StdError + Send + Sync + 'static,
    {
        let header = self.header.ok_or(DataTableError::MissingHeader)?;
        let index = header.require(name, self.row_number)?;
        self.parse_with(index, parser)
    }
}
