//! Nightly-only fixture pinning the native non-strict registry warning.

use rstest_bdd_macros::{given, scenario};

#[given("an unrelated registered step")]
fn unrelated_step() {}

#[scenario(path = "../features/macros/unmatched.feature")]
fn missing_step() {}

compile_error!("nightly registry warning snapshot sentinel");

fn main() {}
