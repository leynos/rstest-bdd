#![cfg(feature = "diagnostics")]
//! Unit tests for registry dumping.

use rstest_bdd::{StepContext, StepExecution, StepKeyword, dump_registry, find_step, step};
use serde_json::Value;

mod common;
use common::noop_wrapper;

step!(StepKeyword::Given, "dump used", noop_wrapper, &[]);
step!(StepKeyword::Given, "dump unused", noop_wrapper, &[]);

#[test]
fn reports_usage_flags() {
    let runner = find_step(StepKeyword::Given, "dump used".into())
        .unwrap_or_else(|| panic!("step not found"));
    match runner(&StepContext::default(), "dump used", None, None) {
        Ok(StepExecution::Continue { .. }) => {}
        Ok(StepExecution::Skipped { .. }) => panic!("step unexpectedly skipped"),
        Err(e) => panic!("execution failed: {e}"),
    }

    let json = dump_registry().unwrap_or_else(|e| panic!("dump registry: {e}"));
    let parsed: Value = serde_json::from_str(&json).unwrap_or_else(|e| panic!("valid json: {e}"));
    let steps = parsed.as_array().unwrap_or_else(|| panic!("array"));
    assert!(
        steps
            .iter()
            .any(|s| s["pattern"] == "dump used" && s["used"] == true)
    );
    assert!(
        steps
            .iter()
            .any(|s| s["pattern"] == "dump unused" && s["used"] == false)
    );
}
