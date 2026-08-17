//! Fixture for the 10.3.3 build-script addition test: a `scenarios!` binding
//! over a directory with a single baseline feature file. The experiment adds
//! a second `.feature` file to the directory and asserts the generated test
//! for it runs — which only happens when the fixture's `build.rs` (written by
//! the test from the documented example) tells Cargo to watch the directory.

use rstest_bdd_macros::{given, scenarios};

#[given("a directory-bound step")]
fn directory_bound_step() {}

scenarios!("tests/features");
