//! Step pattern handling and compilation.
//! This module defines `StepPattern`, a lightweight wrapper around a pattern
//! literal that compiles lazily to a regular expression.

use crate::types::{PlaceholderSyntaxError, StepPatternError};
use regex::Regex;
use rstest_bdd_patterns::{PatternError, SpecificityScore, compile_regex_from_pattern};
use std::borrow::Cow;
use std::hash::{Hash, Hasher};
use std::sync::OnceLock;

/// Pattern text used to match a step at runtime.
#[derive(Debug)]
pub struct StepPattern {
    text: &'static str,
    pub(crate) regex: OnceLock<Regex>,
    specificity: OnceLock<SpecificityScore>,
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
            specificity: OnceLock::new(),
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

    /// Return the cached regular expression without checking compilation status.
    ///
    /// # Panics
    /// Panics if called before [`compile`](Self::compile) has succeeded.
    #[must_use]
    #[expect(
        clippy::expect_used,
        reason = "internal method; callers guarantee prior compilation"
    )]
    pub(crate) fn regex_unchecked(&self) -> &Regex {
        self.regex.get().expect("regex accessed before compilation")
    }

    /// Calculate and cache the specificity score for this pattern.
    ///
    /// Used to rank patterns when multiple match the same step text.
    /// Higher scores indicate more specific patterns.
    ///
    /// # Errors
    ///
    /// Returns [`StepPatternError`] if the pattern contains invalid syntax.
    ///
    /// # Notes
    ///
    /// - This operation is idempotent. Subsequent calls after a successful
    ///   calculation are no-ops.
    /// - This method is thread-safe; concurrent calls may race to compute
    ///   the score, but only the first successful value is cached.
    ///
    /// # Examples
    ///
    /// ```
    /// use rstest_bdd::StepPattern;
    ///
    /// let specific = StepPattern::from("overlap apples");
    /// let generic = StepPattern::from("overlap {item}");
    ///
    /// let specific_score = specific.specificity().expect("specific pattern is valid");
    /// let generic_score = generic.specificity().expect("generic pattern is valid");
    /// assert!(specific_score > generic_score);
    /// ```
    pub fn specificity(&self) -> Result<SpecificityScore, StepPatternError> {
        if let Some(score) = self.specificity.get() {
            return Ok(*score);
        }
        let score = SpecificityScore::calculate(self.text)?;
        let _ = self.specificity.set(score);
        Ok(score)
    }
}

impl From<&'static str> for StepPattern {
    fn from(value: &'static str) -> Self {
        Self::new(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ptr;

    #[test]
    #[expect(clippy::expect_used, reason = "test helper validates success path")]
    fn regex_unchecked_returns_cached_regex_after_compilation() {
        let pattern = StepPattern::from("literal text");
        pattern.compile().expect("literal pattern should compile");

        // regex_unchecked should return the same instance as regex()
        let re_checked = pattern.regex().expect("regex available after compilation");
        let re_unchecked = pattern.regex_unchecked();

        assert!(ptr::eq(re_checked, re_unchecked));
        assert!(re_unchecked.is_match("literal text"));
    }

    #[test]
    #[should_panic(expected = "regex accessed before compilation")]
    fn regex_unchecked_panics_without_prior_compilation() {
        let pattern = StepPattern::from("literal text");
        // This should panic because compile() was never called
        let _ = pattern.regex_unchecked();
    }
}
