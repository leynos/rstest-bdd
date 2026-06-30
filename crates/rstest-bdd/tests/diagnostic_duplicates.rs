//! Behavioural test for duplicate step detection.

use rstest_bdd::{StepKeyword, duplicate_steps, step};

#[path = "common/noop_steps.rs"]
mod noop_steps;
use noop_steps::{noop_async_wrapper, noop_wrapper};

step!(
    StepKeyword::When,
    "diagnostic_duplicate_test_unique",
    noop_wrapper,
    noop_async_wrapper,
    &[]
);
step!(
    StepKeyword::When,
    "diagnostic_duplicate_test_unique",
    noop_wrapper,
    noop_async_wrapper,
    &[]
);

#[test]
fn finds_duplicates() {
    let groups = duplicate_steps();
    assert!(groups.iter().any(|g| g.len() >= 2));
}
