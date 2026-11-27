//! Pass fixture pinning the documented opt-out for the stable marker channel.
//!
//! The crate denies `deprecated`, so this compiles only because the module
//! that holds the invocation also carries `#[allow(deprecated)]`. Removing the
//! allow must break the fixture: the marker is a sibling item of the generated
//! test, which a function-level allow cannot reach.

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

#[allow(deprecated)]
mod opted_out {
    //! The documented opt-out shape around one macro invocation.

    use rstest_bdd_macros::scenario;

    use super::nested;

    #[scenario(
        path = "basic.feature",
        harness = nested::deep::rstest_bdd_harness_tokio::TokioHarness,
    )]
    fn with_nested_tokio_harness() {}
}

fn main() {}