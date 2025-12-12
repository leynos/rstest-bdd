//! Regression tests ensuring wrapper inputs use unique identifiers.

use rstest_bdd::{
    StepContext, StepKeyword, assert_step_err, assert_step_ok, find_step, lookup_step,
};
use rstest_bdd_macros::given;
use std::sync::Mutex;

static CAPTURED_TEXT: Mutex<Option<String>> = Mutex::new(None);

#[given("{text} arrives")]
#[expect(
    clippy::expect_used,
    reason = "tests deliberately panic when the capture mutex is poisoned"
)]
fn capture_text(ctx: &str, text: String) {
    assert_eq!(ctx, "fixture ctx");
    *CAPTURED_TEXT.lock().expect("capture mutex poisoned") = Some(text);
}

#[test]
#[expect(
    clippy::expect_used,
    reason = "test asserts deterministic macro expansion and registry lookups"
)]
fn wrapper_handles_text_capture_without_shadowing() {
    let mut ctx = StepContext::default();
    let fixture = "fixture ctx";
    ctx.insert("ctx", &fixture);

    let step_text = "message arrives";
    let step_fn = find_step(StepKeyword::Given, step_text.into())
        .expect("step should be registered for '{text} arrives'");
    *CAPTURED_TEXT.lock().expect("capture mutex poisoned") = None;

    let _ = assert_step_ok!(step_fn(&mut ctx, step_text, None, None));

    let captured = CAPTURED_TEXT
        .lock()
        .expect("capture mutex poisoned")
        .clone();
    assert_eq!(captured.as_deref(), Some("message"));
}

#[test]
#[expect(
    clippy::expect_used,
    reason = "test inspects placeholder mismatch error formatting"
)]
fn placeholder_mismatch_reports_original_step_text() {
    let mut ctx = StepContext::default();
    let step_fn = lookup_step(StepKeyword::Given, "{text} arrives".into())
        .expect("step should be registered for '{text} arrives'");

    let err = assert_step_err!(step_fn(&mut ctx, "arrives", None, None));
    let display = err.to_string();
    assert!(
        display.contains("arrives"),
        "error should include original step text: {display}"
    );
}
