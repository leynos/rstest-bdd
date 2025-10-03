//! Step pattern handling and compilation.
//! This module defines `StepPattern`, a lightweight wrapper around a pattern
//! literal that compiles lazily to a regular expression.

use crate::types::{PlaceholderSyntaxError, StepPatternError};
use regex::Regex;
use rstest_bdd_patterns::{PatternError, compile_regex_from_pattern};
use std::borrow::Cow;
use std::hash::{Hash, Hasher};
use std::sync::OnceLock;

/// Pattern text used to match a step at runtime.
#[derive(Debug)]
pub struct StepPattern {
    text: &'static str,
    pub(crate) regex: OnceLock<Regex>,
}

// Equality and hashing are by the underlying literal text. This allows
// `&'static StepPattern` to be used as a stable map key while keeping
// semantics intuitive and independent of allocation identity.
impl PartialEq for StepPattern {
    fn eq(&self, other: &Self) -> bool {
        self.text == other.text
    }
}

impl Eq for StepPattern {}

impl Hash for StepPattern {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.text.hash(state);
    }
}

impl From<PatternError> for StepPatternError {
    fn from(err: PatternError) -> Self {
        match err {
            PatternError::Placeholder(info) => Self::PlaceholderSyntax(
                PlaceholderSyntaxError::new(info.message, info.position, info.placeholder),
            ),
            PatternError::Regex(e) => Self::InvalidPattern(e),
        }
    }
}

impl StepPattern {
    /// Create a new pattern wrapper from a string literal.
    #[must_use]
    pub const fn new(value: &'static str) -> Self {
        Self {
            text: value,
            regex: OnceLock::new(),
        }
    }

    /// Access the underlying pattern string.
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        self.text
    }

    /// Compile the pattern into a regular expression, caching the result.
    ///
    /// # Errors
    /// Returns an error if the pattern contains invalid placeholders or the
    /// generated regex fails to compile.
    ///
    /// # Notes
    /// - This operation is idempotent. Subsequent calls after a successful
    ///   compilation are no-ops.
    /// - This method is thread-safe; concurrent calls may race to build a
    ///   `Regex`, but only the first successful value is cached.
    pub fn compile(&self) -> Result<(), StepPatternError> {
        if self.regex.get().is_some() {
            return Ok(());
        }
        let regex = compile_regex_from_pattern(self.text)?;
        let _ = self.regex.set(regex);
        Ok(())
    }

    /// Return the cached regular expression.
    ///
    /// # Examples
    ///
    /// ```
    /// use rstest_bdd::StepPattern;
    ///
    /// let pattern = StepPattern::from("literal text");
    /// assert!(pattern.regex().is_err());
    /// pattern.compile().expect("literal patterns compile");
    /// let regex = pattern.regex().expect("regex available after compilation");
    /// assert!(regex.is_match("literal text"));
    /// ```
    ///
    /// # Errors
    /// Returns [`StepPatternError::NotCompiled`] if [`compile`](Self::compile)
    /// was not invoked beforehand.
    #[must_use = "check whether compilation succeeded"]
    pub fn regex(&self) -> Result<&Regex, StepPatternError> {
        self.regex.get().ok_or(StepPatternError::NotCompiled {
            pattern: Cow::Borrowed(self.text),
        })
    }
}

impl From<&'static str> for StepPattern {
    fn from(value: &'static str) -> Self {
        Self::new(value)
    }
}
