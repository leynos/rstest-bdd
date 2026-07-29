//! Compile-fail fixture: duplicate `#[from]` attributes are rejected.

use rstest_bdd_macros::given;

#[given("a duplicated fixture override")]
fn step(#[from(alpha)] #[from(beta)] value: u32) {}

fn main() {}
