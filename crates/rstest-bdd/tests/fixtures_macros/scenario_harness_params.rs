//! Compile-time fixture validating that `#[scenario]` accepts `harness` and
//! `attributes` parameters with valid types.
use rstest_bdd_macros::{given, scenario, then, when};

#[given("a precondition")]
fn precondition() {}

#[when("an action occurs")]
fn action() {}

#[then("a result is produced")]
fn result() {}

#[scenario(
    path = "basic.feature",
    harness = rstest_bdd_harness::StdHarness,
    attributes = rstest_bdd_harness::DefaultAttributePolicy,
)]
fn with_harness_and_attributes() {}

#[scenario(
    path = "basic.feature",
    harness = rstest_bdd_harness::StdHarness,
)]
fn with_harness_only() {}

#[scenario(
    path = "basic.feature",
    attributes = rstest_bdd_harness::DefaultAttributePolicy,
)]
fn with_attributes_only() {}

// Compile-time guard: fail fast if the feature path changes.
const _: &str = include_str!("basic.feature");

fn main() {}
