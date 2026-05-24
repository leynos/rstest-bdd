//! Compile-pass fixture validating that `scenarios!` accepts an explicit
//! default attribute-policy override for the first-party GPUI harness.
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
    attributes = rstest_bdd_harness::DefaultAttributePolicy,
);

fn main() {}
