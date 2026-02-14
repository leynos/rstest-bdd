//! Compile-fail fixture: `attributes` type does not implement `AttributePolicy`.
use rstest_bdd_macros::{given, scenario, then, when};

struct NotAPolicy;

#[given("a precondition")]
fn precondition() {}

#[when("an action occurs")]
fn action() {}

#[then("a result is produced")]
fn result() {}

#[scenario(
    path = "basic.feature",
    attributes = NotAPolicy,
)]
fn bad_attributes() {}

const _: &str = include_str!("basic.feature");

fn main() {}
