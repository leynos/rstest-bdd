//! Step definitions used by `trybuild` fixtures.
//!
//! This module declares dummy Given/When/Then functions so the
//! procedural macros can register steps for compile tests.
use rstest_bdd::StepError;
use rstest_bdd_macros::{given, when, then};

#[given("a precondition")]
fn precondition() -> Result<(), StepError> {
    Ok(())
}

#[when("an action occurs")]
fn action() -> Result<(), StepError> {
    Ok(())
}

#[then("a result is produced")]
fn result() -> Result<(), StepError> {
    Ok(())
}


fn main() {}
