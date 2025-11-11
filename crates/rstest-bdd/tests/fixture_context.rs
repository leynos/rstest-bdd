//! Behavioural test for fixture context injection

use rstest_bdd::localization::{strip_directional_isolates, ScopedLocalization};
use rstest_bdd::{
    assert_step_err, assert_step_ok, lookup_step, StepContext, StepError, StepKeyword,
};
use rstest_bdd_macros::given;
use unic_langid::langid;

/// Step that asserts the injected `number` fixture equals 42.
#[given("a value")]
#[allow(clippy::trivially_copy_pass_by_ref)]
fn needs_value(number: &u32) {
    assert_eq!(*number, 42);
}

#[given("a panicking value step")]
#[allow(clippy::trivially_copy_pass_by_ref)]
fn panicking_value_step(number: &u32) -> Result<(), String> {
    let _ = number;
    panic!("boom happened")
}

#[test]
#[expect(clippy::expect_used, reason = "step lookup must succeed for test")]
fn context_passes_fixture() {
    let number = 42u32;
    let mut ctx = StepContext::default();
    ctx.insert("number", &number);
    let step_fn = lookup_step(StepKeyword::Given, "a value".into())
        .expect("step 'a value' not found in registry");
    let _ = assert_step_ok!(step_fn(&ctx, "a value", None, None));
}

#[test]
#[expect(clippy::expect_used, reason = "step lookup must succeed for test")]
fn context_missing_fixture_returns_error() {
    let ctx = StepContext::default();
    let step_fn = lookup_step(StepKeyword::Given, "a value".into())
        .expect("step 'a value' not found in registry");
    let err = assert_step_err!(step_fn(&ctx, "a value", None, None));
    let display = strip_directional_isolates(&err.to_string());
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
fn context_missing_fixture_localizes_error() {
    let guard = ScopedLocalization::new(&[langid!("fr")])
        .unwrap_or_else(|error| panic!("failed to scope French locale: {error}"));
    let ctx = StepContext::default();
    let step_fn = lookup_step(StepKeyword::Given, "a value".into())
        .expect("step 'a value' not found in registry");
    let err = assert_step_err!(step_fn(&ctx, "a value", None, None));
    let display = strip_directional_isolates(&err.to_string());
    assert!(display.contains("La fixture « number »"));
    drop(guard);
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
            assert_eq!(message, "boom happened");
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn insert_value_overrides_fixture() {
    let number = 1u32;
    let mut ctx = StepContext::default();
    ctx.insert("number", &number);

    let first = ctx.insert_value(Box::new(5u32));
    assert!(
        first.is_none(),
        "first override should insert without prior value"
    );

    let second = ctx.insert_value(Box::new(7u32));
    let Some(prev) = second else {
        panic!("expected previous override to be returned");
    };
    let Ok(prev) = prev.downcast::<u32>() else {
        panic!("override should downcast to u32");
    };
    assert_eq!(*prev, 5);

    let retrieved: Option<&u32> = ctx.get("number");
    assert_eq!(retrieved, Some(&7));
}

#[test]
fn insert_value_ignored_when_type_not_unique() {
    let one = 1u32;
    let two = 2u32;
    let mut ctx = StepContext::default();
    ctx.insert("one", &one);
    ctx.insert("two", &two);

    let result = ctx.insert_value(Box::new(5u32));
    assert!(result.is_none(), "ambiguous override should be ignored");
    assert_eq!(ctx.get::<u32>("one"), Some(&1));
    assert_eq!(ctx.get::<u32>("two"), Some(&2));
}
