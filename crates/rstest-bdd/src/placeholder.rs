//! Placeholder extraction and pattern-to-regex compilation.
//! This module re-exports the shared pattern engine and exposes the
//! runtime-facing helper for capturing placeholders from steps.

use crate::pattern::StepPattern;
use crate::types::{PlaceholderError, StepText};
use rstest_bdd_patterns::extract_captured_values;

/// Extract placeholder values from a step string using a pattern.
///
/// The runtime uses this helper to materialise step arguments before
/// invoking the registered implementation. Patterns must have been
/// precompiled via [`StepPattern::compile`].
///
/// # Errors
/// - [`PlaceholderError::PatternMismatch`]: the provided text does not match
///   the pattern.
/// - [`PlaceholderError::InvalidPlaceholder`]: pattern contains malformed
///   placeholders.
/// - [`PlaceholderError::InvalidPattern`]: generated regular expression failed
///   to compile.
/// - [`PlaceholderError::NotCompiled`]: the compiled regex was requested before
///   the pattern was compiled.
pub fn extract_placeholders(
    pattern: &StepPattern,
    text: StepText<'_>,
) -> Result<Vec<String>, PlaceholderError> {
    pattern.compile()?;
    let re = pattern.regex()?;
    extract_captured_values(re, text.as_str()).ok_or(PlaceholderError::PatternMismatch)
}
