//! Compile-pass fixture mirroring the third-party harness cookbook shape.
//!
//! The fixture uses a tiny Bevy-like `World` stand-in so the macro contract is
//! validated without adding a real framework dependency to this workspace.

use rstest_bdd_harness::{
    AttributePolicy, HarnessAdapter, HarnessResult, ScenarioRunRequest, TestAttribute,
};
use rstest_bdd_macros::{given, scenario, then, when};

#[derive(Default)]
pub struct World {
    entities: usize,
}

impl World {
    fn spawn_empty(&mut self) {
        self.entities += 1;
    }
}

#[derive(Default)]
pub struct BevyHarness;

impl HarnessAdapter for BevyHarness {
    type Context = World;

    fn run<T>(&self, request: ScenarioRunRequest<'_, Self::Context, T>) -> HarnessResult<T> {
        Ok(request.run(World::default()))
    }
}

pub struct BevyAttributePolicy;

const BEVY_TEST_ATTRIBUTES: [TestAttribute; 1] = [TestAttribute::new("rstest::rstest")];

impl AttributePolicy for BevyAttributePolicy {
    fn test_attributes() -> &'static [TestAttribute] {
        &BEVY_TEST_ATTRIBUTES
    }
}

#[given("a precondition")]
fn precondition(#[from(rstest_bdd_harness_context)] world: &World) {
    assert_eq!(world.entities, 0);
}

#[when("an action occurs")]
fn action(#[from(rstest_bdd_harness_context)] world: &mut World) {
    world.spawn_empty();
}

#[then("a result is produced")]
fn result(#[from(rstest_bdd_harness_context)] world: &World) {
    assert_eq!(world.entities, 1);
}

#[scenario(
    path = "basic.feature",
    harness = BevyHarness,
    attributes = BevyAttributePolicy,
)]
fn third_party_harness_cookbook_example() {}

// Compile-time guard: fail fast if the feature path changes.
const _: &str = include_str!("basic.feature");

fn main() {}
