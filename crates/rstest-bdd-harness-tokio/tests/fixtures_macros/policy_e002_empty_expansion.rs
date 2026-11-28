//! Pass fixture: the alias plus a harness over a feature with no scenarios.
//!
//! `scenarios!` generates no test here, so nothing could have failed per
//! scenario before the diagnostics existed. E002 must therefore not fire:
//! this invocation compiled before and must keep compiling.

use rstest_bdd_macros::{given, scenarios, then, when};

#[given("a precondition")]
fn precondition() {}

#[when("an action occurs")]
fn action() {}

#[then("a result is produced")]
fn result() {}

scenarios!(
    "empty_policy_e002.feature",
    runtime = "tokio-current-thread",
    harness = rstest_bdd_harness_tokio::TokioHarness,
);

fn main() {}
