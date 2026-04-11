//! Compile-pass fixture validating that `#[scenario]` accepts a first-party
//! Tokio harness without an explicit attribute policy.
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
)]
fn with_tokio_harness_default_attributes() {}

const _: &str = include_str!("basic.feature");

fn main() {}
