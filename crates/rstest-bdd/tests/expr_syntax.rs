//! Integration test verifying `expr = "..."` attribute syntax works.
//!
//! This syntax is provided for cucumber-rs migration compatibility.

use rstest::rstest;
use rstest_bdd::{Step, StepKeyword, iter};
use rstest_bdd_macros::{given, scenario, then, when};
use std::cell::Cell;

thread_local! {
    static COUNTER: Cell<i32> = const { Cell::new(0) };
}

/// Step using `expr = "..."` syntax (cucumber-rs style).
#[given(expr = "a counter initialised to zero")]
fn counter_initialised() {
    COUNTER.set(0);
}

/// Step using `expr = "..."` syntax with placeholder.
#[when(expr = "the counter is incremented by {amount}")]
fn counter_incremented(amount: i32) {
    COUNTER.set(COUNTER.get() + amount);
}

/// Step using `expr = "..."` syntax for verification.
#[then(expr = "the counter equals {expected}")]
fn counter_equals(expected: i32) {
    assert_eq!(COUNTER.get(), expected);
}

#[scenario(path = "tests/features/expr_syntax.feature")]
fn expr_syntax_scenario() {}

// Verify that expr syntax registers steps identically to direct literal syntax.
#[rstest]
#[case(StepKeyword::Given, "a counter initialised to zero")]
#[case(StepKeyword::When, "the counter is incremented by {amount}")]
#[case(StepKeyword::Then, "the counter equals {expected}")]
#[case(StepKeyword::When, "an alias result returns ok")]
fn expr_syntax_registers_steps(#[case] keyword: StepKeyword, #[case] pattern: &str) {
    assert!(
        iter::<Step>
            .into_iter()
            .any(|s| s.keyword == keyword && s.pattern.as_str() == pattern),
        "Step not registered: {} {}",
        keyword.as_str(),
        pattern
    );
}

type AliasResult<T> = Result<T, &'static str>;

/// Step using `expr = "...", result` syntax to test combined override.
#[when(expr = "an alias result returns ok", result)]
#[expect(
    clippy::unnecessary_wraps,
    reason = "step intentionally returns a Result alias to exercise return-kind override"
)]
fn alias_result_with_expr_syntax() -> AliasResult<()> {
    Ok(())
}
