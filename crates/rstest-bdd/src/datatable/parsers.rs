//! Helper parsers that support the datatable runtime.

use std::error::Error as StdError;
use std::fmt;

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
#[derive(Debug, Clone, PartialEq, Eq)]
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

impl fmt::Display for TruthyBoolError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "unrecognised boolean value '{value}' (expected yes/y/true/1 or no/n/false/0)",
            value = self.value
        )
    }
}

impl StdError for TruthyBoolError {}

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
        .map_err(|err| TrimmedParseError::new(value.to_string(), err))
}

/// Error returned when [`trimmed`] fails to parse the value.
pub struct TrimmedParseError<E> {
    source: E,
    original_input: String,
}

impl<E> TrimmedParseError<E> {
    pub(crate) fn new(original_input: String, source: E) -> Self {
        Self {
            source,
            original_input,
        }
    }

    #[must_use]
    pub fn original_input(&self) -> &str {
        &self.original_input
    }
}

impl<E> fmt::Debug for TrimmedParseError<E>
where
    E: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TrimmedParseError")
            .field("source", &self.source)
            .field("original_input", &self.original_input)
            .finish()
    }
}

impl<E> fmt::Display for TrimmedParseError<E>
where
    E: fmt::Display,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "failed to parse trimmed value from input '{}': {}",
            self.original_input, self.source
        )
    }
}

impl<E> StdError for TrimmedParseError<E> where E: StdError + 'static {}
