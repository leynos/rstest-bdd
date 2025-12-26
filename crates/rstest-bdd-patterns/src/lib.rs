//! Shared step-pattern parsing utilities for rstest-bdd.
//!
//! The crate exposes placeholder parsing helpers reused by both the runtime
//! and proc-macro crates so they can share validation logic without duplicating
//! the regex construction code paths.

mod capture;
mod errors;
mod hint;
mod keyword;
pub mod pattern;
mod specificity;

pub use capture::extract_captured_values;
pub use errors::{PatternError, PlaceholderErrorInfo};
pub use hint::get_type_pattern;
pub use keyword::{StepKeyword, StepKeywordParseError, UnsupportedStepType};
pub use pattern::build_regex_from_pattern;
pub use specificity::SpecificityScore;

/// Build and compile a `Regex` from a step pattern.
///
/// # Errors
/// Returns [`PatternError`] if the pattern translation or regex compilation fails.
///
/// # Examples
/// ```
/// use rstest_bdd_patterns::compile_regex_from_pattern;
/// let regex = compile_regex_from_pattern("Given {n:u32}").unwrap();
/// assert!(regex.is_match("Given 42"));
/// ```
pub fn compile_regex_from_pattern(pat: &str) -> Result<regex::Regex, PatternError> {
    let src = build_regex_from_pattern(pat)?;
    regex::Regex::new(&src).map_err(PatternError::from)
}
