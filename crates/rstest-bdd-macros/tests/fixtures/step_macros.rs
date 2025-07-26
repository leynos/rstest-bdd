use rstest_bdd_macros::{given, when, then};

#[given("a precondition")]
fn precondition() {}

#[when("an action occurs")]
fn action() {}

#[then("a result is produced")]
fn result() {}


fn main() {}
