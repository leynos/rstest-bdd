//! Shared helpers for placeholder parsing integration tests.

use rstest_bdd::{
    PlaceholderSyntaxError,
    StepPattern,
    StepPatternError,
    StepText,
    extract_placeholders,
};

/// Compile a placeholder pattern for use in assertions.
///
/// # Example
///
/// ```no_run
/// use rstest_bdd::{StepText, extract_placeholders};
/// use support::compiled;
///
/// let pat = compiled("value {n:u32}");
/// let caps = extract_placeholders(&pat, StepText::from("value 42"));
/// assert!(caps.is_ok());
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

/// Assert that `pattern` captures exactly `expected` from `text`.
///
/// # Example
///
/// ```no_run
/// use support::{assert_captures, compiled};
///
/// assert_captures(&compiled("value {n:u32}"), "value 42", &["42"]);
/// ```
///
/// # Panics
/// Panics if the text does not match or the captures differ.
pub fn assert_captures(pattern: &StepPattern, text: &'static str, expected: &[&str]) {
    let Ok(captures) = extract_placeholders(pattern, StepText::from(text)) else {
        panic!("expected {text:?} to match the pattern");
    };
    assert_eq!(captures, expected, "unexpected captures for {text:?}");
}

/// Assert that `pattern` does not match `text`.
///
/// # Example
///
/// ```no_run
/// use support::{assert_no_match, compiled};
///
/// assert_no_match(&compiled("value {n:u32}"), "value none");
/// ```
///
/// # Panics
/// Panics if the text unexpectedly matches.
pub fn assert_no_match(pattern: &StepPattern, text: &'static str) {
    assert!(
        extract_placeholders(pattern, StepText::from(text)).is_err(),
        "expected {text:?} not to match the pattern",
    );
}
