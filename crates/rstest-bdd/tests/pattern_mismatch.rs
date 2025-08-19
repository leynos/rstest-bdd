//! Behavioural test verifying wrapper error propagation on mismatched text

use rstest_bdd::{StepContext, StepKeyword, lookup_step};
use rstest_bdd_macros::given;

#[given("number {value:u32}")]
fn number(value: u32) {
    let _ = value;
}

#[test]
fn returns_error_on_pattern_mismatch() {
    let step_fn = lookup_step(StepKeyword::Given, "number {value:u32}".into())
        .unwrap_or_else(|| panic!("step missing"));
    let ctx = StepContext::default();
    let Err(err) = step_fn(&ctx, "unrelated text", None, None) else {
        panic!("expected mismatch to error");
    };
    assert!(err.contains("does not match pattern"), "{err}");
}
