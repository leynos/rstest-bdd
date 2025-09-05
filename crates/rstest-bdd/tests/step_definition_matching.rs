//! Behavioural tests for step lookup matching.

use std::sync::atomic::{AtomicUsize, Ordering};

use rstest_bdd::{StepContext, StepKeyword, find_step};
use rstest_bdd_macros::given;

static GENERIC_CALLED: AtomicUsize = AtomicUsize::new(0);
static SPECIFIC_CALLED: AtomicUsize = AtomicUsize::new(0);

#[given("a unique step")]
fn unique_step() {}

#[given("overlap {item}")]
#[expect(clippy::needless_pass_by_value, reason = "step consumes the argument")]
fn generic_step(item: String) {
    let _ = item;
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
    let step_fn = find_step(StepKeyword::Given, "a unique step".into())
        .unwrap_or_else(|| panic!("step not found"));
    let ctx = StepContext::default();
    if let Err(e) = step_fn(&ctx, "a unique step", None, None) {
        panic!("unexpected error: {e:?}");
    }
}

#[test]
fn find_step_runs_one_of_multiple_matches() {
    GENERIC_CALLED.store(0, Ordering::Relaxed);
    SPECIFIC_CALLED.store(0, Ordering::Relaxed);
    let step_fn = find_step(StepKeyword::Given, "overlap apples".into())
        .unwrap_or_else(|| panic!("step not found"));
    let ctx = StepContext::default();
    if let Err(e) = step_fn(&ctx, "overlap apples", None, None) {
        panic!("unexpected error: {e:?}");
    }
    let total = GENERIC_CALLED.load(Ordering::Relaxed) + SPECIFIC_CALLED.load(Ordering::Relaxed);
    assert_eq!(total, 1);
}
