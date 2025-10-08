//! Helper parsers that support the datatable runtime.

use std::error::Error as StdError;

use thiserror::Error;

/// Parses boolean values in a tolerant, human-friendly fashion.
///
/// `truthy_bool` recognises common affirmative and negative forms, returning a
/// [`TruthyBoolError`] when the value cannot be classified.
///
/// # Examples
/// ```
/// # use rstest_bdd::datatable::truthy_bool;
/// assert!(truthy_bool("yes").unwrap());
/// assert!(!truthy_bool("no").unwrap());
/// ```
///
/// # Errors
///
/// Returns [`TruthyBoolError`] when the input does not match a recognised form.
pub fn truthy_bool(value: &str) -> Result<bool, TruthyBoolError> {
    match value.trim().to_ascii_lowercase().as_str() {
        "yes" | "y" | "true" | "1" => Ok(true),
        "no" | "n" | "false" | "0" => Ok(false),
        other => Err(TruthyBoolError {
            value: other.to_string(),
        }),
    }
}

/// Error returned when [`truthy_bool`] fails to classify a value.

#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[error("unrecognised boolean value '{value}' (expected yes/y/true/1 or no/n/false/0)")]
pub struct TruthyBoolError {
    value: String,
}

impl TruthyBoolError {
    /// Returns the original, unclassified input.
    #[must_use]
    pub fn value(&self) -> &str {
        &self.value
    }
}

/// Trims leading and trailing whitespace before parsing a value.
///
/// `trimmed` delegates to [`FromStr`] implementations after normalising the
/// input. Errors from the inner parser are preserved.
///
/// # Examples
/// ```
/// # use rstest_bdd::datatable::trimmed;
/// let value: i32 = trimmed(" 42 ").unwrap();
/// assert_eq!(value, 42);
/// ```
///
/// # Errors
///
/// Returns [`TrimmedParseError`] when parsing the trimmed value fails.
pub fn trimmed<T>(value: &str) -> Result<T, TrimmedParseError<T::Err>>
where
    T: std::str::FromStr,
    T::Err: StdError + Send + Sync + 'static,
{
    let trimmed = value.trim();
    trimmed
        .parse()
        .map_err(|source| TrimmedParseError::new(value.to_string(), source))
}

/// Error returned when [`trimmed`] fails to parse the value.

#[derive(Debug, Error)]
#[error("failed to parse trimmed value from input '{original_input}': {source}")]
pub struct TrimmedParseError<E>
where
    E: StdError + Send + Sync + 'static,
{
    original_input: String,
    #[source]
    source: E,
}

impl<E> TrimmedParseError<E>
where
    E: StdError + Send + Sync + 'static,
{
    pub(crate) fn new(original_input: String, source: E) -> Self {
        Self {
            original_input,
            source,
        }
    }

    #[must_use]
    pub fn original_input(&self) -> &str {
        &self.original_input
    }
}
