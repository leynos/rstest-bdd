//! Behavioural test for duplicate step detection.

use rstest_bdd::{StepKeyword, duplicate_steps, step};

mod common;
use common::noop_wrapper;

step!(StepKeyword::When, "duplicate", noop_wrapper, &[]);
step!(StepKeyword::When, "duplicate", noop_wrapper, &[]);

#[test]
fn finds_duplicates() {
    let groups = duplicate_steps();
    assert!(groups.iter().any(|g| g.len() == 2));
}
