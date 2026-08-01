//! Nightly-only UI fixture pinning one proc-macro fallback warning.

use rstest_bdd_macros::{given, scenario, then, when};

mod alias {
    //! Deliberately re-exports the Tokio harness crate so the macro receives a
    //! non-canonical path and exercises the fallback diagnostic.

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
fn with_aliased_tokio_harness() {}

compile_error!("nightly warning snapshot sentinel");

fn main() {}
