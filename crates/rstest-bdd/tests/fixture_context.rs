//! Behavioural test for fixture context injection

use rstest_bdd::{
    StepContext, StepError, StepKeyword, assert_step_err, assert_step_ok, lookup_step,
};
use rstest_bdd_macros::given;

/// Step that asserts the injected `number` fixture equals 42.
#[given("a value")]
#[expect(
    clippy::trivially_copy_pass_by_ref,
    reason = "fixture requires reference"
)]
fn needs_value(number: &u32) {
    assert_eq!(*number, 42);
}

#[given("a panicking value step")]
#[expect(
    clippy::trivially_copy_pass_by_ref,
    reason = "fixture requires reference"
)]
fn panicking_value_step(number: &u32) -> Result<(), String> {
    let _ = number;
    panic!("boom")
}

#[test]
#[expect(clippy::expect_used, reason = "step lookup must succeed for test")]
fn context_passes_fixture() {
    let number = 42u32;
    let mut ctx = StepContext::default();
    ctx.insert("number", &number);
    let step_fn = lookup_step(StepKeyword::Given, "a value".into())
        .expect("step 'a value' not found in registry");
    assert_step_ok!(step_fn(&ctx, "a value", None, None));
}

#[test]
#[expect(clippy::expect_used, reason = "step lookup must succeed for test")]
fn context_missing_fixture_returns_error() {
    let ctx = StepContext::default();
    let step_fn = lookup_step(StepKeyword::Given, "a value".into())
        .expect("step 'a value' not found in registry");
    let err = assert_step_err!(step_fn(&ctx, "a value", None, None));
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
#[expect(clippy::expect_used, reason = "step lookup must succeed for test")]
fn fixture_step_panic_returns_panic_error() {
    let number = 1u32;
    let mut ctx = StepContext::default();
    ctx.insert("number", &number);
    let step_fn = lookup_step(StepKeyword::Given, "a panicking value step".into())
        .expect("step 'a panicking value step' not found in registry");
    let err = assert_step_err!(step_fn(&ctx, "a panicking value step", None, None), "boom");
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

#[test]
fn insert_value_overrides_fixture() {
    let number = 1u32;
    let mut ctx = StepContext::default();
    ctx.insert("number", &number);
    ctx.insert_value(Box::new(5u32));
    let retrieved: Option<&u32> = ctx.get("number");
    assert_eq!(retrieved, Some(&5));
}

#[test]
fn insert_value_ignored_when_type_not_unique() {
    let one = 1u32;
    let two = 2u32;
    let mut ctx = StepContext::default();
    ctx.insert("one", &one);
    ctx.insert("two", &two);
    ctx.insert_value(Box::new(5u32));
    assert_eq!(ctx.get::<u32>("one"), Some(&1));
    assert_eq!(ctx.get::<u32>("two"), Some(&2));
}
