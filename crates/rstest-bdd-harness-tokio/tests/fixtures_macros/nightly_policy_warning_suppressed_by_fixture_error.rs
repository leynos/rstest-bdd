//! Nightly-only UI fixture pinning that a rejected boundary stays silent.
//!
//! The non-canonical harness path would raise W001, but the `Result`-typed
//! fixture with a unit return rejects the expansion first, and a boundary
//! error must suppress that boundary's warning on both channels.

use rstest_bdd_macros::{given, scenario, then, when};

mod alias {
    //! Deliberately re-exports the Tokio harness crate so the macro receives a
    //! non-canonical path and would raise W001.

    // The expansion is rejected before it references the path, so the import
    // is deliberately unused; keeping the warning out of the snapshot leaves
    // it asserting only the diagnostics this fixture is about.
    #[allow(unused_imports)]
    pub use rstest_bdd_harness_tokio;
}

#[given("a precondition")]
fn precondition() {}

#[when("an action occurs")]
fn action() {}

#[then("a result is produced")]
fn result() {}

#[scenario(
    path = "basic.feature",
    harness = alias::rstest_bdd_harness_tokio::TokioHarness,
)]
fn rejected_by_fallible_fixture(world: Result<u32, String>) {}

compile_error!("nightly warning snapshot sentinel");

fn main() {}