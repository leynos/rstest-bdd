//! Compile-fail fixture pinning W003 for two selectors naming different
//! frameworks.
//!
//! The harness selects Tokio and the explicit policy selects GPUI. The policy
//! takes precedence, so the generated test carries no Tokio attribute; the
//! warning says so and offers both remedies.
#![deny(deprecated)]

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

/// The `scenarios!` shape: the alias implies Tokio, the policy names GPUI.
mod alias_shape {
    //! A separate module keeps the two W003 blocks apart in stderr.

    use rstest_bdd_macros::scenarios;

    scenarios!(
        "tests/features/auto",
        runtime = "tokio-current-thread",
        attributes = rstest_bdd_harness_gpui::GpuiAttributePolicy,
    );
}

const _: &str = include_str!("basic.feature");

fn main() {}
