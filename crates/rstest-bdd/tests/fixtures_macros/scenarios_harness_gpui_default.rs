//! Compile-pass fixture validating that `scenarios!` accepts a first-party
//! GPUI harness without an explicit attribute policy.
use rstest_bdd_macros::{given, scenarios, then, when};

#[given("a precondition")]
fn precondition() {}

#[when("an action occurs")]
fn action() {}

#[then("a result is produced")]
fn result() {}

scenarios!(
    "tests/features/auto",
    harness = rstest_bdd_harness_gpui::GpuiHarness,
);

fn main() {}
