//! Compile-time fixture asserting index-based selection reports bounds.
use rstest_bdd_macros::scenario;

#[scenario(path = "basic.feature", index = 2)]
fn index_out_of_range() {}

const _: &str = include_str!("basic.feature");

fn main() {}
