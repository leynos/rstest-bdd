//! Compile-pass fixture mirroring the third-party harness cookbook shape.
//!
//! The fixture uses a tiny Bevy-like `World` stand-in so the macro contract is
//! validated without adding a real framework dependency to this workspace.

use rstest_bdd_harness::{
    AttributePolicy, HarnessAdapter, HarnessResult, ScenarioRunRequest, TestAttribute,
};
use rstest_bdd_macros::{given, scenario, then, when};

/// Minimal stand-in for `bevy::ecs::world::World` in the cookbook example.
///
/// The fixture keeps this type local so the compile contract can exercise a
/// typed harness context without adding Bevy as a workspace dependency.
#[derive(Default)]
pub struct World {
    entities: usize,
}

impl World {
    /// Records one spawned entity for the cookbook's mutable context step.
    fn spawn_empty(&mut self) {
        self.entities += 1;
    }
}

/// Public harness type shaped like a third-party adapter crate export.
#[derive(Default)]
pub struct BevyHarness;

/// Harness adapter implementation that supplies `World` to cookbook steps.
impl HarnessAdapter for BevyHarness {
    /// Scenario context type shared by the cookbook step functions.
    type Context = World;

    /// Runs a scenario request with a fresh empty `World`.
    fn run<T>(&self, request: ScenarioRunRequest<'_, Self::Context, T>) -> HarnessResult<T> {
        Ok(request.run(World::default()))
    }
}

/// Public attribute policy type shaped like a third-party adapter export.
pub struct BevyAttributePolicy;

/// Attributes returned by the cookbook policy implementation.
const BEVY_TEST_ATTRIBUTES: [TestAttribute; 1] = [TestAttribute::new("rstest::rstest")];

/// Attribute policy implementation used by the cookbook scenario macro.
impl AttributePolicy for BevyAttributePolicy {
    /// Returns the test attributes applied to generated cookbook tests.
    fn test_attributes() -> &'static [TestAttribute] {
        &BEVY_TEST_ATTRIBUTES
    }
}

/// Verifies that the harness supplied a fresh cookbook world.
#[given("a precondition")]
fn precondition(#[from(rstest_bdd_harness_context)] world: &World) {
    assert_eq!(world.entities, 0);
}

/// Mutates the cookbook world through the reserved harness context fixture.
#[when("an action occurs")]
fn action(#[from(rstest_bdd_harness_context)] world: &mut World) {
    world.spawn_empty();
}

/// Confirms that the mutation from the `when` step remains visible.
#[then("a result is produced")]
fn result(#[from(rstest_bdd_harness_context)] world: &World) {
    assert_eq!(world.entities, 1);
}

/// Compile-checked scenario using the public cookbook harness and policy.
#[scenario(
    path = "basic.feature",
    harness = BevyHarness,
    attributes = BevyAttributePolicy,
)]
fn third_party_harness_cookbook_example() {}

/// Compile-time guard that fails fast if the feature path changes.
const _: &str = include_str!("basic.feature");

/// Binary entry point required by the trybuild compile-pass fixture.
fn main() {}
