//! Behavioural tests for the `scenarios!` macro.

use rstest_bdd_macros::{given, scenarios, then, when};

#[given("a precondition")]
fn precondition() {}

#[when("an action occurs")]
fn action() {}

#[when("an action occurs with {n}")]
fn action_with_num(n: i32) {
    let _ = n;
}

#[then("events are recorded")]
fn events_recorded() {}

#[then("only fast examples run")]
fn only_fast_examples_run(num: &'static str) {
    assert_eq!(num, "1", "unexpected example row executed");
}

#[when("a slow action occurs")]
fn slow_action_occurs() {
    panic!("slow scenario should be filtered out");
}

#[then("slow events are recorded")]
fn slow_events_recorded() {
    panic!("slow scenario should be filtered out");
}

scenarios!("tests/features/auto");
scenarios!("tests/features/filtered", tags = "@fast");
