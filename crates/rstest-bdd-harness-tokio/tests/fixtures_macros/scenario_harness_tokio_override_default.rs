//! Compile-pass fixture validating that an explicit default attribute policy
//! can override a first-party Tokio harness-led default.
use rstest_bdd_macros::{given, scenario, then, when};

#[given("a precondition")]
fn precondition() {}

#[when("an action occurs")]
fn action() {}

#[then("a result is produced")]
fn result() {}

#[scenario(
    path = "basic.feature",
    harness = rstest_bdd_harness_tokio::TokioHarness,
    attributes = rstest_bdd_harness::DefaultAttributePolicy,
)]
fn with_tokio_harness_default_override() {}

const _: &str = include_str!("basic.feature");

fn main() {}
