//! Compile-fail fixture for mixed immutable and mutable fixture borrows.

use rstest_bdd_macros::given;

#[given("mixed mutability fixtures")]
fn mixed_mutability_fixtures(_read_only: &ReadOnlyFixture, _mutable: &mut MutableFixture) {}

struct MutableFixture;
struct ReadOnlyFixture;

fn main() {}
