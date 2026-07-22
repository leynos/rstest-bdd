//! Compile-pass fixture for mixed immutable and mutable fixture borrows.
//!
//! Before ADR-010 this was a compile-fail case: the generated wrapper held
//! a shared borrow of the context while requesting a mutable one, tripping
//! `E0502`. Guard-based borrowing makes the wrapper compile; this fixture
//! pins that capability.

use rstest_bdd_macros::given;

#[given("mixed mutability fixtures")]
fn mixed_mutability_fixtures(_read_only: &ReadOnlyFixture, _mutable: &mut MutableFixture) {}

struct MutableFixture;
struct ReadOnlyFixture;

fn main() {}
