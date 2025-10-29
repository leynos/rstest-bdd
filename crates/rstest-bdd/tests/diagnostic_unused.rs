//! Behavioural tests for step usage diagnostics.

use rstest_bdd::{find_step, step, unused_steps, StepContext, StepExecution, StepKeyword};

mod common;
use common::noop_wrapper;

step!(StepKeyword::Given, "a used step", noop_wrapper, &[]);
step!(StepKeyword::Given, "an unused step", noop_wrapper, &[]);

#[test]
fn reports_unused_steps() {
    let runner = find_step(StepKeyword::Given, "a used step".into())
        .unwrap_or_else(|| panic!("step not found"));
    match runner(&StepContext::default(), "a used step", None, None) {
        Ok(StepExecution::Continue { .. }) => {}
        Ok(StepExecution::Skipped { .. }) => panic!("step unexpectedly skipped"),
        Err(e) => panic!("execution failed: {e}"),
    }

    let patterns: Vec<_> = unused_steps().iter().map(|s| s.pattern.as_str()).collect();
    assert!(patterns.contains(&"an unused step"));
    assert!(!patterns.contains(&"a used step"));
}
