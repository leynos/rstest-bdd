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
        let mut rows = table.into_iter();
        let header = take_header::<T>(&mut rows)?;
        // A consumed header occupies row 1, so the data rows are numbered from 2.
        let first_row_number = if header.is_some() { 2 } else { 1 };
        let parsed_rows = parse_rows::<T>(rows, header.as_ref(), first_row_number)?;
        Ok(Self(parsed_rows))
    }
}

/// Consume the header row when the row type declares one.
fn take_header<T: DataTableRow>(
    rows: &mut impl Iterator<Item = Vec<String>>,
) -> Result<Option<HeaderSpec>, DataTableError> {
    if !T::REQUIRES_HEADER {
        return Ok(None);
    }
    let header_row = rows.next().ok_or(DataTableError::MissingHeader)?;
    HeaderSpec::new(header_row).map(Some)
}

/// Parse every data row, rejecting a row whose width differs from the header's.
fn parse_rows<T: DataTableRow>(
    rows: impl Iterator<Item = Vec<String>>,
    header: Option<&HeaderSpec>,
    first_row_number: usize,
) -> Result<Vec<T>, DataTableError> {
    let (lower, _) = rows.size_hint();
    let mut parsed_rows = Vec::with_capacity(lower);
    for (index, row) in rows.enumerate() {
        let row_number = first_row_number + index;
        ensure_row_width(header, row.len(), row_number)?;
        let spec = RowSpec::new(header, row_number, index, row);
        parsed_rows.push(T::parse_row(spec)?);
    }
    Ok(parsed_rows)
}

/// Reject a data row whose width differs from the header's column count.
fn ensure_row_width(
    header: Option<&HeaderSpec>,
    actual: usize,
    row_number: usize,
) -> Result<(), DataTableError> {
    let Some(header) = header else {
        return Ok(());
    };
    if actual == header.len() {
        return Ok(());
    }
    Err(DataTableError::UnevenRow {
        row_number,
        expected: header.len(),
        actual,
    })
}
