//! Behavioural test for step registry

use rstest_bdd::{iter, step, Step};

fn sample() {}

step!("When", "behavioural", sample);

#[test]
fn step_is_registered() {
    let found = iter::<Step>
        .into_iter()
        .any(|step| step.pattern == "behavioural" && step.keyword == "When");
    assert!(found, "expected step not found");
}
