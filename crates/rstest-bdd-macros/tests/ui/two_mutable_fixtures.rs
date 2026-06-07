//! Compile-fail fixture for two mutable fixture borrows.

use rstest_bdd_macros::given;

#[given("two mutable fixtures")]
fn two_mutable_fixtures(_first: &mut FirstFixture, _second: &mut SecondFixture) {}

struct FirstFixture;
struct SecondFixture;

fn main() {}
