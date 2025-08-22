//! Tests for step registration via macros

use rstest_bdd::StepError;
use rstest_bdd::{Step, iter};
use rstest_bdd_macros::{given, then, when};

#[given("a precondition")]
#[expect(
    clippy::unnecessary_wraps,
    reason = "step functions must return StepError"
)]
fn precondition() -> Result<(), StepError> {
    Ok(())
}

#[when("an action")]
#[expect(
    clippy::unnecessary_wraps,
    reason = "step functions must return StepError"
)]
fn action() -> Result<(), StepError> {
    Ok(())
}

#[then("a result")]
#[expect(
    clippy::unnecessary_wraps,
    reason = "step functions must return StepError"
)]
fn result() -> Result<(), StepError> {
    Ok(())
}

#[test]
fn macros_register_steps() {
    let cases = [
        (rstest_bdd::StepKeyword::Given, "a precondition"),
        (rstest_bdd::StepKeyword::When, "an action"),
        (rstest_bdd::StepKeyword::Then, "a result"),
    ];

    for (keyword, pattern) in cases {
        assert!(
            iter::<Step>
                .into_iter()
                .any(|s| s.keyword == keyword && s.pattern.as_str() == pattern),
            "Step not registered: {} {}",
            keyword.as_str(),
            pattern
        );
    }
}
