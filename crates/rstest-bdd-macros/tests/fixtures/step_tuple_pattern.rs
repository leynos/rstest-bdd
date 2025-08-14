//! Compile-fail fixture: tuple destructuring in a step parameter must emit a
//! "complex destructuring patterns are not yet supported" error, enforcing the
//! single bare identifier rule.

use rstest_bdd_macros::given;

#[given("coordinates")]
fn step_with_tuple((x, y): (i32, i32)) {}

fn main() {}
