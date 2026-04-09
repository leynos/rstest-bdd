//! Compile-pass fixture validating that `scenarios!` accepts the canonical
//! GPUI attribute-policy path.
use rstest_bdd_macros::{given, scenarios, then, when};

#[given("a precondition")]
fn precondition() {}

#[when("an action occurs")]
fn action() {}

#[then("a result is produced")]
fn result() {}

scenarios!(
    "tests/features/auto",
    attributes = rstest_bdd_harness_gpui::GpuiAttributePolicy,
);

fn main() {}
