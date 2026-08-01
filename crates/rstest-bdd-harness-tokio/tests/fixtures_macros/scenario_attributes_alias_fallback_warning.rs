//! Compile-fail fixture pinning the fallback diagnostic for an aliased Tokio
//! attribute policy used by `#[scenario]`.
#![deny(deprecated)]

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
    attributes = alias::rstest_bdd_harness_tokio::TokioAttributePolicy,
)]
fn with_aliased_tokio_attribute_policy() {}

const _: &str = include_str!("basic.feature");

fn main() {}
