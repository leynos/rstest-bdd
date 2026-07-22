//! Compile-pass fixture for two mutable fixture borrows.
//!
//! Before ADR-010 this was a compile-fail case: `StepContext::borrow_mut`
//! took `&mut self`, so the generated wrapper could not hold two mutable
//! guards and rejected this signature with `E0499`. Guard-based borrowing
//! makes the wrapper compile; this fixture pins that capability.

use rstest_bdd_macros::given;

#[given("two mutable fixtures")]
fn two_mutable_fixtures(_first: &mut FirstFixture, _second: &mut SecondFixture) {}

struct FirstFixture;
struct SecondFixture;

fn main() {}
