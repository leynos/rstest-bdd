//! Compile-fail fixture ensuring tag filters must match at least one scenario.
use rstest_bdd_macros::scenario;

#[scenario(path = "tagged.feature", tags = "@fast")]
fn tags_must_match() {}

const _: &str = include_str!("tagged.feature");

fn main() {}
