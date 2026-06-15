//! Compile-pass fixture validating generated delegation for harness failures.
use rstest_bdd_harness::FailingHarness;
use rstest_bdd_macros::{given, scenario, then, when};

#[given("a precondition")]
fn precondition() {}

#[when("an action occurs")]
fn action() {}

#[then("a result is produced")]
fn result() {}

#[scenario(
    path = "basic.feature",
    harness = FailingHarness,
)]
fn failing_harness_delegation_compiles() {}

const _: &str = include_str!("basic.feature");

fn main() {}
