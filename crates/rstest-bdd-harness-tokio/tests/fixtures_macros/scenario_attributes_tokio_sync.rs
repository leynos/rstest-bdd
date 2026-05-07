//! Compile-pass fixture validating that `#[scenario]` omits Tokio test
//! attributes for sync functions even when `TokioAttributePolicy` is selected.
use rstest_bdd_macros::{given, scenario, then, when};

#[given("a precondition")]
fn precondition() {}

#[when("an action occurs")]
fn action() {}

#[then("a result is produced")]
fn result() {}

#[scenario(
    path = "basic.feature",
    attributes = rstest_bdd_harness_tokio::TokioAttributePolicy,
)]
fn with_tokio_attributes_policy_sync() {}

// Compile-time guard: fail fast if the feature path changes.
const _: &str = include_str!("basic.feature");

fn main() {}
