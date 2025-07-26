//! Step definitions used by `trybuild` fixtures.
//!
//! This module declares dummy Given/When/Then functions so the
//! procedural macros can register steps for compile tests.
use rstest_bdd_macros::{given, when, then};

#[given("a precondition")]
fn precondition() {}

#[when("an action occurs")]
fn action() {}

#[then("a result is produced")]
fn result() {}


fn main() {}
