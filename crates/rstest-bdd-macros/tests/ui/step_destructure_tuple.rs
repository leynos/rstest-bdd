//! Compile-fail test for tuple pattern arguments.
use rstest_bdd_macros::given;

#[given("coords")]
fn tuple_pattern((x, y): (i32, i32)) {}

fn main() {}
