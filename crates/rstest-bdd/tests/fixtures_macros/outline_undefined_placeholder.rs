//! Compile-time fixture for scenario outline with undefined placeholder.
//!
//! This test verifies that the scenario macro emits an error when step text
//! contains a placeholder that does not match any column in the Examples table.
use rstest_bdd_macros::scenario;

#[scenario(path = "../features/macros/outline_undefined_placeholder.feature")]
fn undefined_placeholder(valid: &'static str) {}

fn main() {}
