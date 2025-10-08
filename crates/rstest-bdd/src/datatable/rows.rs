//! Typed row abstractions used when parsing datatable content.

use std::convert::TryFrom;
use std::ops::Deref;

use derive_more::From;

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

#[derive(Debug, Clone, PartialEq, Eq, From)]
pub struct Rows<T>(Vec<T>);

impl<T> Deref for Rows<T> {
    type Target = [T];

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T> Rows<T> {
    /// Consumes the wrapper, returning the inner [`Vec`].
    #[must_use]
    pub fn into_vec(self) -> Vec<T> {
        self.0
    }
}

impl<T> IntoIterator for Rows<T> {
    type Item = T;
    type IntoIter = std::vec::IntoIter<T>;

    fn into_iter(self) -> Self::IntoIter {
        // Manual impl keeps owned iteration behaviour identical to Vec while the
        // `From<Vec<T>>` derive still removes conversion boilerplate.
        self.0.into_iter()
    }
}

impl<'a, T> IntoIterator for &'a Rows<T> {
    type Item = &'a T;
    type IntoIter = std::slice::Iter<'a, T>;

    fn into_iter(self) -> Self::IntoIter {
        // Delegating to the backing Vec preserves the `for row in &rows` pattern
        // without relying on the derive macro's reference expansion.
        self.0.iter()
    }
}

impl<'a, T> IntoIterator for &'a mut Rows<T> {
    type Item = &'a mut T;
    type IntoIter = std::slice::IterMut<'a, T>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.iter_mut()
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
        let (lower, _) = rows_iter.size_hint();
        let mut parsed_rows = Vec::with_capacity(lower);
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
        Ok(Self(parsed_rows))
    }
}
