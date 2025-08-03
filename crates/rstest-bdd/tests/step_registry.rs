//! Behavioural test for step registry

use rstest_bdd::{Step, StepError, iter, step};

fn sample() {}
#[expect(
    clippy::unnecessary_wraps,
    reason = "required to match StepFn signature"
)]
fn wrapper(ctx: &rstest_bdd::StepContext<'_>, _text: &str) -> Result<(), StepError> {
    // Adapter for zero-argument step functions
    let _ = ctx;
    sample();
    Ok(())
}

step!(rstest_bdd::StepKeyword::When, "behavioural", wrapper, &[]);

#[test]
fn step_is_registered() {
    let found = iter::<Step>.into_iter().any(|step| {
        step.pattern.as_str() == "behavioural" && step.keyword == rstest_bdd::StepKeyword::When
    });
    assert!(found, "expected step not found");
}
