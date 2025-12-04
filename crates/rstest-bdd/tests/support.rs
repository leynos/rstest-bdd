//! Shared helpers for placeholder parsing integration tests.

use rstest_bdd::{PlaceholderSyntaxError, StepPattern, StepPatternError};

/// Compile a placeholder pattern for use in assertions.
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
