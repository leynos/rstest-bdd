//! Compile-time fixture validating that a single step definition binds without ambiguity.
use rstest_bdd_macros::{given, when, then, scenario};

#[given("a precondition")]
fn precondition() {}

#[when("an action occurs")]
fn action() {}

#[then("a result is produced")]
fn result() {}

// trybuild copies this source to a temp dir; use an explicit path back to the feature file.
#[scenario(path = "../../../../crates/rstest-bdd-macros/tests/fixtures/basic.feature")]
fn basic() {}

fn main() {}
