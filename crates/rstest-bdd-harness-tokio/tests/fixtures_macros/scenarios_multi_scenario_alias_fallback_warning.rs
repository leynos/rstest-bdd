//! Compile-fail fixture pinning how many fallback diagnostics `scenarios!`
//! reports when one aliased attribute-policy path feeds several generated
//! scenarios.
//!
//! The feature file declares two scenarios, so this fixture is the coverage
//! that distinguishes per-scenario emission from a single shared diagnostic at
//! the `scenarios!` expansion boundary.
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
    attributes = alias::rstest_bdd_harness_tokio::TokioAttributePolicy,
);

fn main() {}
