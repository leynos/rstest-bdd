//! Behavioural tests verifying wrapper pattern matching and error propagation

use std::sync::atomic::{AtomicU32, Ordering};

use rstest_bdd::{StepContext, StepError, StepKeyword, lookup_step};
use rstest_bdd_macros::given;

static CAPTURED: AtomicU32 = AtomicU32::new(0);

#[given("number {value:u32}")]
fn number(value: u32) {
    CAPTURED.store(value, Ordering::Relaxed);
}

#[test]
fn passes_captured_value() {
    CAPTURED.store(0, Ordering::Relaxed);
    #[expect(clippy::expect_used, reason = "step registered above")]
    let step_fn =
        lookup_step(StepKeyword::Given, "number {value:u32}".into()).expect("step missing");
    let ctx = StepContext::default();
    #[expect(clippy::expect_used, reason = "matching text should succeed")]
    step_fn(&ctx, "number 41", None, None).expect("step should match");
    assert_eq!(CAPTURED.load(Ordering::Relaxed), 41);
}

#[test]
fn returns_error_on_pattern_mismatch() {
    #[expect(clippy::expect_used, reason = "step registered above")]
    let step_fn =
        lookup_step(StepKeyword::Given, "number {value:u32}".into()).expect("step missing");
    let ctx = StepContext::default();
    let Err(err) = step_fn(&ctx, "unrelated text", None, None) else {
        panic!("expected mismatch to error");
    };
    match err {
        StepError::ExecutionError { pattern, function, message } => {
            assert_eq!(pattern, "number {value:u32}");
            assert_eq!(function, "number");
            assert!(message.contains("does not match pattern"));
            assert!(message.contains("unrelated text"));
            assert!(message.contains("number {value:u32}"));
        }
        other => panic!("unexpected error: {other:?}"),
    }
}
