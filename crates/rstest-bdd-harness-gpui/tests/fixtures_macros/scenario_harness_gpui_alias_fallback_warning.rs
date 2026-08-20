//! Stable UI fixture pinning the GPUI adapter fallback diagnostic.
//!
//! The re-exported path has canonical GPUI crate evidence but is not the
//! canonical crate-root path, so the macro falls back to the base harness API.
#![deny(deprecated)]

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

const _: &str = include_str!("basic.feature");

fn main() {}
