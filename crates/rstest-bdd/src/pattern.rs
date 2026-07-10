//! Step pattern handling and compilation.
//! This module defines `StepPattern`, a lightweight wrapper around a pattern
//! literal that compiles lazily to a regular expression.

use crate::types::{PlaceholderSyntaxError, StepPatternError};
use regex::Regex;
use rstest_bdd_patterns::{PatternError, SpecificityScore, compile_regex_from_pattern};
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
        self.compiled_regex().map(|_| ())
    }

    /// Return the compiled regular expression, compiling it on first use.
    ///
    /// # Errors
    /// Returns an error if the pattern contains invalid placeholders or the
    /// generated regex fails to compile.
    pub(crate) fn compiled_regex(&self) -> Result<&Regex, StepPatternError> {
        if let Some(regex) = self.regex.get() {
            return Ok(regex);
        }
        let regex = compile_regex_from_pattern(self.text)?;
        // A concurrent compilation may have won the race; `get_or_init`
        // returns whichever value was cached first.
        Ok(self.regex.get_or_init(|| regex))
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
    //! Unit tests for step pattern compilation and caching.

    use super::*;
    use std::ptr;

    #[test]
    #[expect(clippy::expect_used, reason = "test helper validates success path")]
    fn compiled_regex_returns_cached_regex_after_compilation() {
        let pattern = StepPattern::from("literal text");
        pattern.compile().expect("literal pattern should compile");

        // Repeated calls return the same cached instance
        let re1 = pattern.compiled_regex().expect("regex should be cached");
        let re2 = pattern.compiled_regex().expect("regex should be cached");

        assert!(ptr::eq(re1, re2));
        assert!(re1.is_match("literal text"));
    }

    #[test]
    #[expect(clippy::expect_used, reason = "test validates compilation")]
    fn compile_is_idempotent() {
        let pattern = StepPattern::from("literal text");

        // First compile succeeds
        pattern.compile().expect("literal pattern should compile");
        let re1 = pattern.compiled_regex().expect("regex should be cached");

        // Second compile is a no-op and returns the same regex
        pattern.compile().expect("recompile should succeed");
        let re2 = pattern.compiled_regex().expect("regex should be cached");

        assert!(ptr::eq(re1, re2), "compile should be idempotent");
    }

    #[test]
    fn regex_is_not_cached_without_prior_compilation() {
        let pattern = StepPattern::from("literal text");
        // No call to compile() has happened, so nothing is cached yet.
        assert!(pattern.regex.get().is_none());
    }
}
