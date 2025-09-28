//! Compile-fail fixture: struct destructuring in a step parameter must emit an
//! "unsupported pattern" error, enforcing the single identifier rule.

use rstest_bdd_macros::given;

struct User {
    name: String,
}

#[given("user data")]
fn step_with_struct(User { name }: User) {}

fn main() {}
