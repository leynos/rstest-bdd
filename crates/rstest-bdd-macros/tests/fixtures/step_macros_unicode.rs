//! Step definitions with Unicode identifiers used by trybuild tests, including
//! mixed ASCII/non-ASCII and digit-prefixed names.
#![expect(
    non_snake_case,
    reason = "Unicode identifiers in fixtures are intentional and not snake_case",
)]
use rstest_bdd_macros::{given, then, when};

#[given("précondition")]
fn précondition() {}

#[when("acción")]
fn acción() {}

#[then("résultat")]
fn résultat() {}

// Mixed ASCII and non-ASCII characters.
#[given("stepé")]
fn stepé() {}

// Step name starting with a digit and containing a space.
#[when("1er pas")]
fn _1er_pas() {}

// Intentional CamelCase identifier to satisfy lint expectation.
#[then("CamelCase")]
fn CamelCase() {}

// Unicode-only function name and emoji in label to stress sanitization.
#[then("done ✅")]
fn 数字() {}

fn main() {}
