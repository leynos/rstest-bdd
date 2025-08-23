//! Behavioural tests verifying wrapper pattern matching and error propagation

use std::sync::atomic::{AtomicU32, Ordering};

use rstest_bdd::{StepContext, StepKeyword, lookup_step};
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
    assert!(err.contains("does not match pattern"), "{err}");
    assert!(err.contains("unrelated text"), "{err}");
    assert!(err.contains("number {value:u32}"), "{err}");
}
