//! Compile-pass fixture proving `#[harness_context]` and the legacy
//! `#[from(rstest_bdd_harness_context)]` spelling coexist in one suite.

use rstest_bdd_harness::{
    AttributePolicy, HarnessAdapter, HarnessResult, ScenarioRunRequest, TestAttribute,
};
use rstest_bdd_macros::{given, scenario, then, when};

/// Minimal world stand-in so the compile contract exercises a typed harness
/// context without adding a framework dependency.
#[derive(Default)]
pub struct World {
    steps: usize,
}

impl World {
    /// Records one executed step for the coexistence assertions.
    fn record_step(&mut self) {
        self.steps += 1;
    }

    /// How many steps have run so far.
    const fn steps_run(&self) -> usize {
        self.steps
    }
}

/// Public harness type shaped like a first-party adapter export.
#[derive(Default)]
pub struct LocalHarness;

/// Harness adapter that supplies a fresh `World` to every scenario.
impl HarnessAdapter for LocalHarness {
    /// Scenario context type shared by the step functions.
    type Context = World;

    /// Runs a scenario request with a fresh empty `World`.
    fn run<T>(&self, request: ScenarioRunRequest<'_, Self::Context, T>) -> HarnessResult<T> {
        Ok(request.run(World::default()))
    }
}

/// Public attribute policy type shaped like a first-party adapter export.
pub struct LocalAttributePolicy;

/// Attributes returned by the policy implementation.
const LOCAL_TEST_ATTRIBUTES: [TestAttribute; 1] = [TestAttribute::new("rstest::rstest")];

/// Attribute policy implementation used by the scenario macro.
impl AttributePolicy for LocalAttributePolicy {
    /// Returns the test attributes applied to generated tests.
    fn test_attributes() -> &'static [TestAttribute] {
        &LOCAL_TEST_ATTRIBUTES
    }
}

/// A precondition reached through the readable marker.
#[given("a precondition")]
fn precondition(#[harness_context] world: &World) {
    assert_eq!(world.steps_run(), 0);
}

/// An action reached through the legacy `#[from(...)]` spelling.
#[when("an action occurs")]
fn action(#[from(rstest_bdd_harness_context)] world: &mut World) {
    world.record_step();
}

/// A result reached through the readable marker again.
#[then("a result is produced")]
fn result(#[harness_context] world: &World) {
    assert_eq!(world.steps_run(), 1);
}

/// Compile-checked scenario using the local harness and policy.
#[scenario(
    path = "basic.feature",
    harness = LocalHarness,
    attributes = LocalAttributePolicy,
)]
fn marker_and_legacy_spelling_coexist() {}

/// Compile-time guard that fails fast if the feature path changes.
const _: &str = include_str!("basic.feature");

/// Binary entry point required by the trybuild compile-pass fixture.
fn main() {}
