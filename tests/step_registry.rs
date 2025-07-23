//! Behavioural test for step registry
!
use rstest_bdd::{Step, inventory};

fn sample() {}

inventory::submit! {
    Step::new("When", "behavioural", sample, file!(), line!())
}

#[test]
fn step_is_registered() {
    let found = inventory::iter::<Step>
        .into_iter()
        .any(|step| step.pattern == "behavioural" && step.keyword == "When");
    assert!(found, "expected step not found");
}
