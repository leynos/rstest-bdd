//! Step definitions with Unicode identifiers used by trybuild tests, including
//! mixed ASCII/non-ASCII and digit-prefixed names.
#![allow(non_snake_case)] // wrapper names for digit-prefixed steps violate snake case
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

fn main() {}
