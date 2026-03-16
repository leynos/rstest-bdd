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

// Force a compile error so trybuild captures the deprecation warning emitted by the macro.
compile_error!("forced failure");

fn main() {}
