//! Nightly-only fixture pinning the native W003 warning.

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
    attributes = rstest_bdd_harness_gpui::GpuiAttributePolicy,
)]
fn tokio_harness_with_gpui_policy() {}

compile_error!("nightly W003 warning snapshot sentinel");

fn main() {}
