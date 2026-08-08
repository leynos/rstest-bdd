//! Nightly-only UI fixture pinning how many native fallback warnings
//! `scenarios!` reports for one aliased attribute-policy path.
//!
//! The feature file declares two scenarios. Nightly emits the diagnostic
//! through `proc_macro::Diagnostic` rather than the stable `#[deprecated]`
//! marker, so this fixture is the nightly counterpart to
//! `scenarios_multi_scenario_alias_fallback_warning.rs`.

use rstest_bdd_macros::{given, scenarios, then, when};

mod alias {
    //! Deliberately re-exports the Tokio harness crate so the macro receives a
    //! non-canonical path and exercises the fallback diagnostic.

    pub use rstest_bdd_harness_tokio;
}

#[given("a precondition")]
fn precondition() {}

#[when("an action occurs")]
fn action() {}

#[then("an async result is produced")]
async fn async_result() {}

scenarios!(
    "scenarios_multi_fallback.feature",
    attributes = alias::rstest_bdd_harness_tokio::TokioAttributePolicy,
);

compile_error!("nightly multi-scenario warning snapshot sentinel");

fn main() {}
