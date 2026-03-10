//! Clippy UI fixture validating underscore-prefixed scenario fixture parameters
//! under `-D clippy::used_underscore_binding`.

#![deny(clippy::used_underscore_binding)]

use rstest::fixture;
use rstest_bdd_macros::{given, scenario, then, when};

#[derive(Default)]
struct HarnessWorld {
    started: bool,
}

#[fixture]
fn harness_world() -> HarnessWorld {
    HarnessWorld::default()
}

#[given("a configured harness world")]
fn configured_world(#[from(harness_world)] _harness_world: &HarnessWorld) {}

#[when("the harness starts")]
fn start_harness(#[from(harness_world)] harness_world: &mut HarnessWorld) {
    harness_world.started = true;
}

#[then("startup succeeds")]
fn startup_succeeds(#[from(harness_world)] harness_world: &HarnessWorld) {
    assert!(harness_world.started);
}

#[scenario(
    path = "features/lint_underscore_binding.feature",
    name = "Start harness with valid configuration"
)]
fn start_harness_with_valid_configuration(_harness_world: HarnessWorld) {}

fn main() {}
