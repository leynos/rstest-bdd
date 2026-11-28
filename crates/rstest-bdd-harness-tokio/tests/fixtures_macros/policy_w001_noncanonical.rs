//! Compile-fail fixture pinning W001 for two non-canonical Tokio paths.
//!
//! Both the harness and the attribute policy reach the adapter crate through
//! a two-level re-export, so neither is recognized. The generated test is
//! synchronous, so canonicalizing either path would emit no tokio attribute
//! either: the messages must therefore not claim one was dropped.
#![deny(deprecated)]

use rstest_bdd_macros::{given, scenario, then, when};

mod nested {
    //! A two-level re-export of the Tokio harness crate.

    pub mod deep {
        pub use rstest_bdd_harness_tokio;
    }
}

#[given("a precondition")]
fn precondition() {}

#[when("an action occurs")]
fn action() {}

#[then("a result is produced")]
fn result() {}

#[scenario(
    path = "basic.feature",
    harness = nested::deep::rstest_bdd_harness_tokio::TokioHarness,
    attributes = nested::deep::rstest_bdd_harness_tokio::TokioAttributePolicy,
)]
fn with_nested_tokio_paths() {}

const _: &str = include_str!("basic.feature");

fn main() {}