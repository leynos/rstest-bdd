//! Error types shared by the pattern parsing modules.

use std::fmt;
use thiserror::Error;

/// Additional context for placeholder-related parsing errors.
///
/// # Examples
/// ```
/// use rstest_bdd_patterns::PlaceholderErrorInfo;
/// let info = PlaceholderErrorInfo::new("invalid placeholder", 3, Some("value".into()));
/// assert_eq!(info.placeholder.as_deref(), Some("value"));
/// assert_eq!(info.position, 3);
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlaceholderErrorInfo {
    pub message: &'static str,
    pub position: usize,
    pub placeholder: Option<String>,
}

impl PlaceholderErrorInfo {
    /// Create a new error description for a placeholder failure.
    ///
    /// # Examples
    /// ```
    /// use rstest_bdd_patterns::PlaceholderErrorInfo;
    /// let info = PlaceholderErrorInfo::new("invalid", 1, None);
    /// assert_eq!(info.message, "invalid");
    /// ```
    #[must_use]
    pub fn new(message: &'static str, position: usize, placeholder: Option<String>) -> Self {
        Self {
            message,
            position,
            placeholder,
        }
    }
}

impl fmt::Display for PlaceholderErrorInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.placeholder {
            Some(name) => write!(
                f,
                "{} for placeholder `{}` at byte {} (zero-based)",
                self.message, name, self.position
            ),
            None => write!(f, "{} at byte {} (zero-based)", self.message, self.position),
        }
    }
}

/// Errors surfaced while converting step patterns into regular expressions.
///
/// # Examples
/// ```
/// use rstest_bdd_patterns::{PatternError, PlaceholderErrorInfo};
/// let info = PlaceholderErrorInfo::new("invalid", 2, Some("count".into()));
/// let err = PatternError::Placeholder(info.clone());
/// assert_eq!(err.to_string(), info.to_string());
/// ```
#[derive(Debug, Error)]
pub enum PatternError {
    #[error("{0}")]
    Placeholder(PlaceholderErrorInfo),
    #[error(transparent)]
    Regex(regex::Error),
}

pub(crate) fn placeholder_error(
    message: &'static str,
    position: usize,
    placeholder: Option<String>,
) -> PatternError {
    PatternError::Placeholder(PlaceholderErrorInfo::new(message, position, placeholder))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_placeholder_with_name() {
        let info = PlaceholderErrorInfo::new("invalid", 4, Some("count".into()));
        assert_eq!(
            info.to_string(),
            "invalid for placeholder `count` at byte 4 (zero-based)"
        );
    }

    #[test]
    fn formats_placeholder_without_name() {
        let info = PlaceholderErrorInfo::new("oops", 1, None);
        assert_eq!(info.to_string(), "oops at byte 1 (zero-based)");
    }

    #[test]
    fn forwards_regex_error_display() {
        let err = PatternError::Regex(regex::Error::Syntax("bad".into()));
        assert_eq!(
            err.to_string(),
            regex::Error::Syntax("bad".into()).to_string()
        );
    }
}
