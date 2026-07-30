//! Compile-pass fixture for fallible GPUI scenarios with non-`Debug` errors.

#![deny(warnings)]

use rstest_bdd_macros::{given, scenario, then, when};

struct NonDebugError;

#[given("a precondition")]
fn precondition() {}

#[when("an action occurs")]
fn action() {}

#[then("a result is produced")]
fn result() {}

#[scenario(
    path = "basic.feature",
    harness = rstest_bdd_harness_gpui::GpuiHarness,
)]
fn fallible_sync_scenario() -> Result<(), NonDebugError> {
    Ok(())
}

#[scenario(
    path = "basic.feature",
    attributes = rstest_bdd_harness_gpui::GpuiAttributePolicy,
)]
async fn fallible_async_scenario() -> Result<(), NonDebugError> {
    Ok(())
}

const _: &str = include_str!("basic.feature");

fn main() {}
