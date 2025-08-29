//! Behavioural tests for step usage diagnostics.

use rstest_bdd::{find_step, step, unused_steps, StepContext, StepError, StepKeyword};

#[expect(clippy::unnecessary_wraps, reason = "wrapper must match StepFn signature")]
fn used_wrapper(
    ctx: &StepContext<'_>,
    _text: &str,
    _docstring: Option<&str>,
    _table: Option<&[&[&str]]>,
) -> Result<(), StepError> {
    let _ = ctx;
    Ok(())
}

step!(StepKeyword::Given, "a used step", used_wrapper, &[]);

#[expect(clippy::unnecessary_wraps, reason = "wrapper must match StepFn signature")]
fn unused_wrapper(
    ctx: &StepContext<'_>,
    _text: &str,
    _docstring: Option<&str>,
    _table: Option<&[&[&str]]>,
) -> Result<(), StepError> {
    let _ = ctx;
    Ok(())
}

step!(StepKeyword::Given, "an unused step", unused_wrapper, &[]);

#[test]
fn reports_unused_steps() {
    let runner = find_step(StepKeyword::Given, "a used step".into())
        .unwrap_or_else(|| panic!("step not found"));
    runner(&StepContext::default(), "a used step", None, None)
        .unwrap_or_else(|e| panic!("execution failed: {e}"));

    let patterns: Vec<_> = unused_steps().iter().map(|s| s.pattern.as_str()).collect();
    assert!(patterns.contains(&"an unused step"));
    assert!(!patterns.contains(&"a used step"));
}
