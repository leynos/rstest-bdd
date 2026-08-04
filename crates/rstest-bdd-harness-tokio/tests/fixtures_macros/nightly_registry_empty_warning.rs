//! Nightly-only fixture pinning the native empty-registry warning.

use rstest_bdd_macros::scenario;

#[scenario(path = "basic.feature")]
fn registry_without_definitions() {}

compile_error!("nightly empty registry warning snapshot sentinel");

fn main() {}
