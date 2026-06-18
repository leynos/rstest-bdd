//! Test fixture pinning the first-party adapter fallback diagnostic.
//!
//! The harness path goes through a local re-export module, so the macro
//! cannot identify it as the first-party Tokio adapter and falls back to
//! resolving base API types through `rstest-bdd-harness`. The macro surfaces
//! this on stable toolchains as a `deprecated` warning carrying the fallback
//! guidance; `deny(deprecated)` promotes it to an error so `trybuild` pins
//! the message text.
#![deny(deprecated)]

use rstest_bdd_macros::{given, scenario, then, when};

mod alias {
    pub use rstest_bdd_harness_tokio::TokioHarness;
}

#[given("a precondition")]
fn precondition() {}

#[when("an action occurs")]
fn action() {}

#[then("a result is produced")]
fn result() {}

#[scenario(
    path = "basic.feature",
    harness = alias::TokioHarness,
)]
fn with_aliased_tokio_harness() {}

const _: &str = include_str!("basic.feature");

fn main() {}
