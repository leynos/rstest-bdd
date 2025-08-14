//! Compile-fail fixture: tuple destructuring in a step parameter must emit an
//! "unsupported pattern" error, enforcing the single identifier rule.

use rstest_bdd_macros::given;

#[given("coordinates")]
fn step_with_tuple((x, y): (i32, i32)) {}

fn main() {}
