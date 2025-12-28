//! Shared helpers for placeholder parsing integration tests.

use rstest_bdd::{
    PlaceholderSyntaxError, StepPattern, StepPatternError, StepText, extract_placeholders,
};

/// Compile a placeholder pattern for use in assertions.
///
/// # Example
///
/// ```no_run
/// use support::compiled;
///
/// let pat = compiled("value {n:u32}");
/// assert!(pat.regex().unwrap().is_match("value 42"));
/// ```
///
/// # Panics
/// Panics if the pattern fails to compile.
#[must_use]
#[expect(clippy::expect_used, reason = "test helper should fail loudly")]
pub fn compiled(pattern: &'static str) -> StepPattern {
    let pat = StepPattern::from(pattern);
    pat.compile().expect("failed to compile pattern");
    pat
}

/// Compile a pattern and extract placeholders, returning the captured values.
///
/// # Example
///
/// ```no_run
/// use support::compile_and_extract;
///
/// let caps = compile_and_extract("value {n:u32}", "value 42");
/// assert_eq!(caps, vec!["42"]);
/// ```
///
/// # Panics
/// Panics if the pattern fails to compile or if matching fails.
#[must_use]
#[expect(clippy::expect_used, reason = "test helper should fail loudly")]
pub fn compile_and_extract(pattern: &'static str, text: &str) -> Vec<String> {
    let pat = compiled(pattern);
    extract_placeholders(&pat, StepText::from(text)).expect("match expected")
}

/// Expect the provided pattern to emit a placeholder syntax error.
///
/// # Example
///
/// ```no_run
/// use rstest_bdd::StepPattern;
/// use support::expect_placeholder_syntax;
///
/// let err = expect_placeholder_syntax(StepPattern::from("value {n:}"));
/// assert_eq!(err.position, 6);
/// ```
///
/// # Panics
/// Panics if compilation succeeds or returns a different error type.
#[expect(
    clippy::needless_pass_by_value,
    reason = "test helper consumes the placeholder pattern by value"
)]
pub fn expect_placeholder_syntax(pat: StepPattern) -> PlaceholderSyntaxError {
    match pat.compile() {
        Err(StepPatternError::PlaceholderSyntax(e)) => e,
        other => panic!("expected PlaceholderSyntax error, got {other:?}"),
    }
}
