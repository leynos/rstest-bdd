//! Behavioural test for fixture context injection

use rstest_bdd::{StepContext, StepError, StepKeyword};
use rstest_bdd_macros::given;

#[given("a value")]
#[expect(
    clippy::trivially_copy_pass_by_ref,
    reason = "fixture requires reference"
)]
fn needs_value(#[from(number)] number: &u32) {
    assert_eq!(*number, 42);
}

#[test]
fn context_passes_fixture() {
    let number = 42u32;
    let mut ctx = StepContext::default();
    ctx.insert("number", &number);
    let step_fn = rstest_bdd::lookup_step(StepKeyword::Given, "a value".into())
        .unwrap_or_else(|| panic!("step 'a value' not found in registry"));
    let result = step_fn(&ctx, "a value", None, None);
    assert!(result.is_ok(), "step execution failed: {result:?}");
}

#[test]
fn context_missing_fixture_returns_error() {
    let ctx = StepContext::default();
    let step_fn = rstest_bdd::lookup_step(StepKeyword::Given, "a value".into())
        .unwrap_or_else(|| panic!("step 'a value' not found in registry"));
    let result = step_fn(&ctx, "a value", None, None);
    let err = match result {
        Ok(()) => panic!("expected error when fixture is missing"),
        Err(e) => e,
    };
    match err {
        StepError::MissingFixture { name, ty, step } => {
            assert_eq!(name, "number");
            assert_eq!(ty, "u32");
            assert_eq!(step, "needs_value");
        }
        other => panic!("unexpected error: {other:?}"),
    }
}
