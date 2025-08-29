//! Unit tests for registry dumping.

use rstest_bdd::{StepContext, StepError, StepKeyword, dump_registry, find_step, step};
use serde_json::Value;

#[expect(
    clippy::unnecessary_wraps,
    reason = "wrapper must match StepFn signature"
)]
fn wrapper(
    ctx: &StepContext<'_>,
    _text: &str,
    _docstring: Option<&str>,
    _table: Option<&[&[&str]]>,
) -> Result<(), StepError> {
    let _ = ctx;
    Ok(())
}

step!(StepKeyword::Given, "dump used", wrapper, &[]);
step!(StepKeyword::Given, "dump unused", wrapper, &[]);

#[test]
fn reports_usage_flags() {
    let runner = find_step(StepKeyword::Given, "dump used".into())
        .unwrap_or_else(|| panic!("step not found"));
    runner(&StepContext::default(), "dump used", None, None)
        .unwrap_or_else(|e| panic!("execution failed: {e}"));

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
