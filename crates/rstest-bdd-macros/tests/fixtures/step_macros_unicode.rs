//! Step definitions with Unicode identifiers used by trybuild tests.
use rstest_bdd_macros::{given, when, then};

#[given("précondition")]
fn précondition() {}

#[when("acción")]
fn acción() {}

#[then("résultat")]
fn résultat() {}

fn main() {}
