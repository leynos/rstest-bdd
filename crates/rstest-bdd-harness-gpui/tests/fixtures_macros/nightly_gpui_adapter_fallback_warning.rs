//! Nightly UI fixture pinning one native GPUI adapter fallback warning.

use rstest_bdd_macros::{given, scenario, then, when};

mod alias {
    //! Re-exports the GPUI harness crate to exercise adapter fallback warnings.

    pub use rstest_bdd_harness_gpui;
}

#[given("a precondition")]
fn precondition() {}

#[when("an action occurs")]
fn action() {}

#[then("a result is produced")]
fn result() {}

#[scenario(
    path = "basic.feature",
    harness = alias::rstest_bdd_harness_gpui::GpuiHarness,
)]
fn with_aliased_gpui_harness() {}

compile_error!("nightly GPUI fallback warning snapshot sentinel");

fn main() {}
