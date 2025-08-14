//! Compile-fail fixture: nested destructuring in a step parameter must emit an
//! "unsupported pattern" error, enforcing the single identifier rule.

use rstest_bdd_macros::given;

struct User {
    coords: (i32, i32),
}

#[given("user coords")]
fn step_nested(User { coords: (x, y) }: User) {}

fn main() {}
