//! Typed row abstractions used when parsing datatable content.

use std::convert::TryFrom;
use std::ops::Deref;

use super::{DataTableError, HeaderSpec, RowSpec};

/// Trait implemented by types that can be constructed from a [`RowSpec`].
pub trait DataTableRow: Sized {
    /// When `true`, [`Rows<T>`](Rows) expects the first row to contain headers.
    const REQUIRES_HEADER: bool = false;

    /// Parse a row into the target type.
    ///
    /// # Errors
    ///
    /// Implementors should return [`DataTableError`] describing conversion
    /// failures.
    fn parse_row(row: RowSpec<'_>) -> Result<Self, DataTableError>;
}

/// A strongly-typed collection of parsed rows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rows<T>(Vec<T>);

impl<T> Rows<T> {
    pub(super) fn new(rows: Vec<T>) -> Self {
        Self(rows)
    }

    /// Returns the number of parsed rows.
    #[must_use]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Returns `true` when the collection is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Provides shared access to the parsed rows.
    #[must_use]
    pub fn as_slice(&self) -> &[T] {
        &self.0
    }

    /// Returns an iterator over the parsed rows.
    pub fn iter(&self) -> std::slice::Iter<'_, T> {
        self.0.iter()
    }

    /// Consumes the collection, returning the underlying vector.
    #[must_use]
    pub fn into_inner(self) -> Vec<T> {
        self.0
    }
}

impl<T> IntoIterator for Rows<T> {
    type Item = T;
    type IntoIter = std::vec::IntoIter<T>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

impl<T> Deref for Rows<T> {
    type Target = [T];

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<'a, T> IntoIterator for &'a Rows<T> {
    type Item = &'a T;
    type IntoIter = std::slice::Iter<'a, T>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl<T> TryFrom<Vec<Vec<String>>> for Rows<T>
where
    T: DataTableRow,
{
    type Error = DataTableError;

    fn try_from(table: Vec<Vec<String>>) -> Result<Self, Self::Error> {
        let mut rows_iter = table.into_iter();
        let mut row_number = 1;
        let header = if T::REQUIRES_HEADER {
            let header_row = rows_iter.next().ok_or(DataTableError::MissingHeader)?;
            let spec = HeaderSpec::new(header_row)?;
            row_number += 1;
            Some(spec)
        } else {
            None
        };
        let mut parsed_rows = Vec::new();
        for (index, row) in rows_iter.enumerate() {
            if let Some(ref header) = header {
                if row.len() != header.len() {
                    return Err(DataTableError::UnevenRow {
                        row_number: row_number + index,
                        expected: header.len(),
                        actual: row.len(),
                    });
                }
            }
            let spec = RowSpec::new(header.as_ref(), row_number + index, index, row);
            let parsed = T::parse_row(spec)?;
            parsed_rows.push(parsed);
        }
        Ok(Self::new(parsed_rows))
    }
}
