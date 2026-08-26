//! Nightly-only fixture pinning the native runtime deprecation warning.

use rstest_bdd_macros::{given, scenarios, then, when};

#[given("a precondition")]
fn a_precondition() {}

#[when("an action occurs")]
fn an_action_occurs() {}

#[then("a result is produced")]
fn a_result_is_produced() {}

scenarios!(
    "basic.feature",
    runtime = "tokio-current-thread"
);

compile_error!("nightly runtime warning snapshot sentinel");

fn main() {}
