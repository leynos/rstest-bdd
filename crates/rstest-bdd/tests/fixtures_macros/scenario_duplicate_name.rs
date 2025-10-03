//! Compile-time fixture verifying that duplicate scenario titles require an
//! index selector.
use rstest_bdd_macros::{given, scenario, then, when};

#[given("a precondition")]
fn precondition() {}

#[when("an action occurs")]
fn action() {}

#[then("a result is produced")]
fn result() {}

#[scenario(path = "duplicate_titles.feature", name = "Shared title")]
fn duplicate_titles() {}

const _: &str = include_str!("duplicate_titles.feature");

fn main() {}
