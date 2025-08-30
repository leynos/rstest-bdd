//! Behavioural test for duplicate step detection.

use rstest_bdd::{StepContext, StepError, StepKeyword, duplicate_steps, step};

#[expect(
    clippy::unnecessary_wraps,
    reason = "wrapper must match StepFn signature"
)]
fn one(
    ctx: &StepContext<'_>,
    _text: &str,
    _docstring: Option<&str>,
    _table: Option<&[&[&str]]>,
) -> Result<(), StepError> {
    let _ = ctx;
    Ok(())
}

#[expect(
    clippy::unnecessary_wraps,
    reason = "wrapper must match StepFn signature"
)]
fn two(
    ctx: &StepContext<'_>,
    _text: &str,
    _docstring: Option<&str>,
    _table: Option<&[&[&str]]>,
) -> Result<(), StepError> {
    let _ = ctx;
    Ok(())
}

step!(StepKeyword::When, "duplicate", one, &[]);
step!(StepKeyword::When, "duplicate", two, &[]);

#[test]
fn finds_duplicates() {
    let groups = duplicate_steps();
    assert!(groups.iter().any(|g| g.len() == 2));
}
