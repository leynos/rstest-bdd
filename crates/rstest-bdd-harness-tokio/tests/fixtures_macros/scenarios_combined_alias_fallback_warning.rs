//! Compile-fail fixture pinning that `harness` and `attributes` are
//! independent supplied paths.
//!
//! Both are aliased and both qualify, so the contract of one diagnostic per
//! distinct qualifying supplied path means exactly two diagnostics — not one
//! merged diagnostic, and not one per generated scenario. The feature file
//! declares two scenarios, so a per-scenario regression would show four.
#![deny(deprecated)]

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
    harness = alias::rstest_bdd_harness_tokio::TokioHarness,
    attributes = alias::rstest_bdd_harness_tokio::TokioAttributePolicy,
);

fn main() {}
