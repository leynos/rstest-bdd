//! Compile-time fixture validating that a single step definition binds without ambiguity.
use rstest_bdd_macros::{given, when, then, scenario};

#[given("a precondition")]
fn precondition() {}

#[when("an action occurs")]
fn action() {}

#[then("a result is produced")]
fn result() {}

// Resolve the feature relative to this fixture to avoid brittle paths.
#[scenario(path = "../../../../crates/rstest-bdd/tests/fixtures_macros/basic.feature")]
fn basic() {}

// Compile-time guard: fail fast if the feature path changes.
const _: &str = include_str!("../../../../crates/rstest-bdd/tests/fixtures_macros/basic.feature");

fn main() {}
