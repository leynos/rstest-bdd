//! Behavioural tests for the `scenarios!` macro.

use rstest_bdd_macros::{given, scenarios, then, when};

#[given("a precondition")]
fn precondition() {}

#[when("an action occurs")]
fn action() {}

#[when("an action occurs with <num>")]
fn action_with_num() {}

#[then("events are recorded")]
fn events_recorded() {}

scenarios!("tests/features/auto");
