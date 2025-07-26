// Behavioural test for step registry

use rstest_bdd::{Step, iter, step};

fn sample() {}
fn wrapper(ctx: &rstest_bdd::StepContext<'_>) {
    let _ = ctx; // fixture context unused
    sample();
}

step!("When", "behavioural", wrapper, &[]);

#[test]
fn step_is_registered() {
    let found = iter::<Step>
        .into_iter()
        .any(|step| step.pattern == "behavioural" && step.keyword == "When");
    assert!(found, "expected step not found");
}
