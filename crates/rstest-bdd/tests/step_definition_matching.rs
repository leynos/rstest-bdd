//! Behavioural tests for step lookup matching.

use std::sync::atomic::{AtomicUsize, Ordering};

use rstest_bdd::{find_step, StepContext, StepExecution, StepKeyword};
use rstest_bdd_macros::given;

static GENERIC_CALLED: AtomicUsize = AtomicUsize::new(0);
static SPECIFIC_CALLED: AtomicUsize = AtomicUsize::new(0);

#[given("a unique step")]
fn unique_step() {}

#[given("overlap {item}")]
fn generic_step(item: String) {
    let _ = item.into_boxed_str();
    GENERIC_CALLED.fetch_add(1, Ordering::Relaxed);
}

#[given("overlap apples")]
fn specific_step() {
    SPECIFIC_CALLED.fetch_add(1, Ordering::Relaxed);
}

#[test]
fn find_step_returns_none_for_missing() {
    assert!(find_step(StepKeyword::Given, "no match".into()).is_none());
}

#[test]
fn find_step_executes_single_match() {
    #[expect(clippy::expect_used, reason = "test ensures step exists")]
    let step_fn = find_step(StepKeyword::Given, "a unique step".into()).expect("step not found");
    let ctx = StepContext::default();
    match step_fn(&ctx, "a unique step", None, None) {
        Ok(StepExecution::Continue { .. }) => {}
        Ok(StepExecution::Skipped { .. }) => panic!("step unexpectedly skipped"),
        Err(e) => panic!("unexpected error: {e:?}"),
    }
}

#[test]
fn find_step_runs_one_of_multiple_matches() {
    GENERIC_CALLED.store(0, Ordering::Relaxed);
    SPECIFIC_CALLED.store(0, Ordering::Relaxed);
    #[expect(clippy::expect_used, reason = "test ensures step exists")]
    let step_fn = find_step(StepKeyword::Given, "overlap apples".into()).expect("step not found");
    let ctx = StepContext::default();
    match step_fn(&ctx, "overlap apples", None, None) {
        Ok(StepExecution::Continue { .. }) => {}
        Ok(StepExecution::Skipped { .. }) => panic!("step unexpectedly skipped"),
        Err(e) => panic!("unexpected error: {e:?}"),
    }
    let generic = GENERIC_CALLED.load(Ordering::Relaxed);
    let specific = SPECIFIC_CALLED.load(Ordering::Relaxed);
    assert_eq!(
        (generic, specific),
        (0, 1),
        "literal step must win over generic pattern"
    );
}
