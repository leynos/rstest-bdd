use rstest_bdd_macros::{given, when, then, scenario};

#[given("a precondition")]
fn precondition() {}

#[when("an action occurs")]
fn action() {}

#[then("a result is produced")]
fn result() {}

#[scenario("basic.feature")]
fn my_scenario() {}

fn main() {}
