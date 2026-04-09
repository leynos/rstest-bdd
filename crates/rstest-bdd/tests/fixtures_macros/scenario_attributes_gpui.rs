//! Compile-pass fixture validating that `#[scenario]` resolves the canonical
//! GPUI attribute-policy path.
use rstest_bdd_macros::{given, scenario, then, when};

#[given("a precondition")]
fn precondition() {}

#[when("an action occurs")]
fn action() {}

#[then("a result is produced")]
fn result() {}

#[scenario(
    path = "basic.feature",
    attributes = rstest_bdd_harness_gpui::GpuiAttributePolicy,
)]
async fn with_gpui_attributes_policy() {}

// Compile-time guard: fail fast if the feature path changes.
const _: &str = include_str!("basic.feature");

fn main() {}
