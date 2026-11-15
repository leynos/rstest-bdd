//! Typed row abstractions used when parsing datatable content.

use std::{convert::TryFrom, ops::Deref};

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

    fn deref(&self) -> &Self::Target { &self.0 }
}

impl<T> Rows<T> {
    /// Consumes the wrapper, returning the inner [`Vec`].
    #[must_use]
    pub fn into_vec(self) -> Vec<T> { self.0 }
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

    fn into_iter(self) -> Self::IntoIter { self.0.iter_mut() }
}

impl<T> TryFrom<Vec<Vec<String>>> for Rows<T>
where
    T: DataTableRow,
{
    type Error = DataTableError;

    fn try_from(table: Vec<Vec<String>>) -> Result<Self, Self::Error> {
        let mut rows_iter = table.into_iter();
        let header = read_header::<T>(&mut rows_iter)?;
        // Row numbers are one-based; the header, when present, occupies row 1.
        let first_data_row = if header.is_some() { 2 } else { 1 };
        let (lower, _) = rows_iter.size_hint();
        let mut parsed_rows = Vec::with_capacity(lower);
        for (index, row) in rows_iter.enumerate() {
            parsed_rows.push(parse_data_row::<T>(
                header.as_ref(),
                first_data_row + index,
                index,
                row,
            )?);
        }
        Ok(Self(parsed_rows))
    }
}

/// Consume the header row when `T` requires one.
fn read_header<T: DataTableRow>(
    rows: &mut impl Iterator<Item = Vec<String>>,
) -> Result<Option<HeaderSpec>, DataTableError> {
    if !T::REQUIRES_HEADER {
        return Ok(None);
    }
    let header_row = rows.next().ok_or(DataTableError::MissingHeader)?;
    HeaderSpec::new(header_row).map(Some)
}

/// Check a data row against the header width and parse it into `T`.
fn parse_data_row<T: DataTableRow>(
    header: Option<&HeaderSpec>,
    row_number: usize,
    index: usize,
    row: Vec<String>,
) -> Result<T, DataTableError> {
    if let Some(header) = header
        && row.len() != header.len()
    {
        return Err(DataTableError::UnevenRow {
            row_number,
            expected: header.len(),
            actual: row.len(),
        });
    }
    T::parse_row(RowSpec::new(header, row_number, index, row))
}
