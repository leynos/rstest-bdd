//! Runtime coverage for the third-party harness cookbook example.
//!
//! The trybuild fixture proves the cookbook-shaped adapter compiles. This
//! integration test executes the same contract through `#[scenario]`, proving
//! the harness and steps run end-to-end.

use rstest_bdd_harness::{
    AttributePolicy, HarnessAdapter, HarnessResult, ScenarioRunRequest, TestAttribute,
};
use rstest_bdd_macros::{given, scenario, then, when};
use serial_test::serial;
use std::sync::atomic::{AtomicBool, Ordering};

static HARNESS_RAN: AtomicBool = AtomicBool::new(false);
static GIVEN_SAW_EMPTY_WORLD: AtomicBool = AtomicBool::new(false);
static WHEN_MUTATED_WORLD: AtomicBool = AtomicBool::new(false);
static THEN_SAW_MUTATED_WORLD: AtomicBool = AtomicBool::new(false);

/// Minimal `World` stand-in used as the cookbook harness context.
#[derive(Default)]
pub struct World {
    entities: usize,
}

impl World {
    fn spawn_empty(&mut self) {
        self.entities += 1;
    }
}

/// Harness adapter shaped like a public third-party Bevy integration export.
#[derive(Default)]
pub struct BevyHarness;

impl HarnessAdapter for BevyHarness {
    type Context = World;

    fn run<T>(&self, request: ScenarioRunRequest<'_, Self::Context, T>) -> HarnessResult<T> {
        reset_observations();
        HARNESS_RAN.store(true, Ordering::Release);
        Ok(request.run(World::default()))
    }
}

/// Attribute policy shaped like a public third-party Bevy integration export.
pub struct BevyAttributePolicy;

const BEVY_TEST_ATTRIBUTES: [TestAttribute; 1] = [TestAttribute::new("rstest::rstest")];

impl AttributePolicy for BevyAttributePolicy {
    fn test_attributes() -> &'static [TestAttribute] {
        &BEVY_TEST_ATTRIBUTES
    }
}

fn reset_observations() {
    HARNESS_RAN.store(false, Ordering::Release);
    GIVEN_SAW_EMPTY_WORLD.store(false, Ordering::Release);
    WHEN_MUTATED_WORLD.store(false, Ordering::Release);
    THEN_SAW_MUTATED_WORLD.store(false, Ordering::Release);
}

#[given("the cookbook world starts empty")]
fn cookbook_world_starts_empty(#[from(rstest_bdd_harness_context)] world: &World) {
    assert_eq!(world.entities, 0);
    GIVEN_SAW_EMPTY_WORLD.store(true, Ordering::Release);
}

#[when("the cookbook app spawns one entity")]
fn cookbook_app_spawns_one_entity(#[from(rstest_bdd_harness_context)] world: &mut World) {
    world.spawn_empty();
    assert_eq!(world.entities, 1);
    WHEN_MUTATED_WORLD.store(true, Ordering::Release);
}

#[then("the cookbook world contains one entity")]
fn cookbook_world_contains_one_entity(#[from(rstest_bdd_harness_context)] world: &World) {
    assert_eq!(world.entities, 1);
    THEN_SAW_MUTATED_WORLD.store(true, Ordering::Release);
}

#[scenario(
    path = "tests/features/third_party_harness_cookbook.feature",
    harness = BevyHarness,
    attributes = BevyAttributePolicy,
)]
#[serial]
fn third_party_harness_cookbook_runs_end_to_end() {
    assert!(
        HARNESS_RAN.load(Ordering::Acquire),
        "BevyHarness::run should have executed"
    );
    assert!(
        GIVEN_SAW_EMPTY_WORLD.load(Ordering::Acquire),
        "given step should have observed an empty world"
    );
    assert!(
        WHEN_MUTATED_WORLD.load(Ordering::Acquire),
        "when step should have mutated the world"
    );
    assert!(
        THEN_SAW_MUTATED_WORLD.load(Ordering::Acquire),
        "then step should have observed the mutation"
    );
    reset_observations();
}
