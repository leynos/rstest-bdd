//! Compile-time fixture validating that a single step definition binds without ambiguity.
use rstest_bdd_macros::{given, when, then, scenario};

#[given("a precondition")]
fn precondition() {}

#[when("an action occurs")]
fn action() {}

#[then("a result is produced")]
fn result() {}

// trybuild copies this source to a temp dir without the feature file, so this path walks back to the repository; shorter relative paths fail.
#[scenario(path = "../../../../crates/rstest-bdd-macros/tests/fixtures/basic.feature")]
fn basic() {}

// Compile-time guard: fail fast if the feature path changes.
const _: &str = include_str!("../../../../crates/rstest-bdd-macros/tests/fixtures/basic.feature");

fn main() {}
