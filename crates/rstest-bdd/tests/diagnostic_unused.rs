//! Behavioural tests for step usage diagnostics.

use rstest_bdd::{StepContext, StepExecution, StepKeyword, find_step, step, unused_steps};

mod common;
use common::{noop_async_wrapper, noop_wrapper};

step!(
    StepKeyword::Given,
    "a used step",
    noop_wrapper,
    noop_async_wrapper,
    &[]
);
step!(
    StepKeyword::Given,
    "an unused step",
    noop_wrapper,
    noop_async_wrapper,
    &[]
);

#[test]
fn reports_unused_steps() {
    let runner = find_step(StepKeyword::Given, "a used step".into())
        .unwrap_or_else(|| panic!("step not found"));
    let mut ctx = StepContext::default();
    match runner(&mut ctx, "a used step", None, None) {
        Ok(StepExecution::Continue { .. }) => {}
        Ok(StepExecution::Skipped { .. }) => panic!("step unexpectedly skipped"),
        Err(e) => panic!("execution failed: {e}"),
    }

    let patterns: Vec<_> = unused_steps().iter().map(|s| s.pattern.as_str()).collect();
    assert!(patterns.contains(&"an unused step"));
    assert!(!patterns.contains(&"a used step"));
}
