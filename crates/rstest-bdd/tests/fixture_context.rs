//! Behavioural test for fixture context injection

use rstest_bdd::{StepContext, StepError, StepKeyword};
use rstest_bdd_macros::given;

/// Step that asserts the injected `number` fixture equals 42.
#[given("a value")]
#[expect(
    clippy::trivially_copy_pass_by_ref,
    reason = "fixture requires reference"
)]
fn needs_value(#[from(number)] number: &u32) {
    assert_eq!(*number, 42);
}

#[given("a panicking value step")]
#[expect(
    clippy::trivially_copy_pass_by_ref,
    reason = "fixture requires reference"
)]
fn panicking_value_step(#[from(number)] number: &u32) -> Result<(), String> {
    let _ = number;
    panic!("boom")
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
    let display = err.to_string();
    match err {
        StepError::MissingFixture { name, ty, step } => {
            assert_eq!(name, "number");
            assert_eq!(ty, "u32");
            assert_eq!(step, "needs_value");
            assert!(
                display.contains("Missing fixture 'number'"),
                "unexpected Display: {display}"
            );
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn fixture_step_panic_returns_panic_error() {
    let number = 1u32;
    let mut ctx = StepContext::default();
    ctx.insert("number", &number);
    let step_fn = rstest_bdd::lookup_step(StepKeyword::Given, "a panicking value step".into())
        .unwrap_or_else(|| panic!("step 'a panicking value step' not found in registry"));
    let err = match step_fn(&ctx, "a panicking value step", None, None) {
        Ok(()) => panic!("expected panic error"),
        Err(e) => e,
    };
    match err {
        StepError::PanicError {
            pattern,
            function,
            message,
        } => {
            assert_eq!(pattern, "a panicking value step");
            assert_eq!(function, "panicking_value_step");
            assert_eq!(message, "boom");
        }
        other => panic!("unexpected error: {other:?}"),
    }
}
