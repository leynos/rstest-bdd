//! Compile-fail fixture: `DefaultAttributePolicy` without a harness on an
//! async fn must not compile because no async executor is injected.
use rstest_bdd_macros::{given, scenario, then, when};

#[given("a precondition")]
fn precondition() {}

#[when("an action occurs")]
fn action() {}

#[then("a result is produced")]
fn result() {}

// `DefaultAttributePolicy` suppresses #[tokio::test]; no harness provides a
// runtime. An `async fn` scenario therefore has no executor.
#[scenario(
    path = "basic.feature",
    attributes = rstest_bdd_harness::DefaultAttributePolicy,
)]
async fn with_default_policy_only_async() {}

mod my {
    //! A policy the macros cannot evaluate.

    pub struct Policy;
}

mod alias {
    //! A non-canonical route to the Tokio policy.

    pub use rstest_bdd_harness_tokio;
}

// A third-party policy adds no executor either, and the message says why.
#[scenario(path = "basic.feature", attributes = my::Policy)]
async fn with_foreign_policy_only_async() {}

// A non-canonical first-party policy is not recognized, so it adds none.
#[scenario(
    path = "basic.feature",
    attributes = alias::rstest_bdd_harness_tokio::TokioAttributePolicy,
)]
async fn with_evidenced_policy_only_async() {}

// Compile-time guard: fail fast if the feature path changes.
const _: &str = include_str!("basic.feature");

fn main() {}
