//! End-to-end regression test for ADR-010 guard-based borrowing.
//!
//! A single step takes two `&mut` fixture parameters. Before the redesign
//! this could not compile: `StepContext::borrow_mut` took `&mut self`, so the
//! generated wrapper triggered `E0499` when it held two mutable guards at
//! once. With guard-based interior borrowing the wrapper holds both guards
//! concurrently, and mutation is visible to later steps.

use rstest::fixture;
use rstest_bdd_macros::{given, scenario, then, when};

/// Counter mutated by steps; one instance per named fixture.
#[derive(Default)]
struct LeftCounter(u32);

/// Second counter type so each fixture has a distinct concrete type.
#[derive(Default)]
struct RightCounter(u32);

#[fixture]
fn left() -> LeftCounter {
    LeftCounter(10)
}

#[fixture]
fn right() -> RightCounter {
    RightCounter(20)
}

#[given("two counters are available")]
fn counters_available(left: &LeftCounter, right: &RightCounter) {
    assert_eq!(left.0, 10);
    assert_eq!(right.0, 20);
}

#[when("both counters are incremented in one step")]
fn increment_both(left: &mut LeftCounter, right: &mut RightCounter) {
    // Both mutable borrows are alive at the same time inside the generated
    // wrapper — the E0499 case this suite pins.
    left.0 += 1;
    right.0 += 2;
}

#[then("both counters reflect the increments")]
fn counters_reflect_increments(left: &LeftCounter, right: &RightCounter) {
    assert_eq!(left.0, 11);
    assert_eq!(right.0, 22);
}

#[scenario(
    path = "tests/features/concurrent_mut_fixtures.feature",
    name = "One step mutates two fixtures"
)]
fn one_step_mutates_two_fixtures(left: LeftCounter, right: RightCounter) {}

// --- Harness context + world: the GPUI adoption shape ---------------------

use rstest_bdd_harness::{HarnessAdapter, HarnessError, ScenarioRunRequest};

/// Harness-provided context mutated alongside world state.
#[derive(Debug, Default)]
struct CountingContext {
    counter: usize,
}

/// World state mutated in the same step as the harness context.
#[derive(Default)]
struct HarnessWorld {
    entries: Vec<&'static str>,
}

#[derive(Default)]
struct CountingHarness;

impl HarnessAdapter for CountingHarness {
    type Context = CountingContext;

    fn run<T>(&self, request: ScenarioRunRequest<'_, Self::Context, T>) -> Result<T, HarnessError> {
        Ok(request.run(CountingContext::default()))
    }
}

#[fixture]
fn harness_world() -> HarnessWorld {
    HarnessWorld::default()
}

#[given("the harness world starts empty")]
fn harness_world_starts_empty(harness_world: &HarnessWorld) {
    assert!(harness_world.entries.is_empty());
}

#[when("the step mutates harness context and world together")]
fn mutate_context_and_world(
    #[from(rstest_bdd_harness_context)] context: &mut CountingContext,
    harness_world: &mut HarnessWorld,
) {
    // Mutable harness context and mutable world state borrowed from the same
    // `StepContext` concurrently — the shape that previously forced GPUI
    // adopters into thread-local workarounds (design doc section 2.7.6.1).
    context.counter += 1;
    harness_world.entries.push("mutated");
}

#[then("the harness context and world reflect the mutations")]
fn context_and_world_reflect_mutations(
    #[from(rstest_bdd_harness_context)] context: &CountingContext,
    harness_world: &HarnessWorld,
) {
    assert_eq!(context.counter, 1);
    assert_eq!(harness_world.entries, ["mutated"]);
}

#[scenario(
    path = "tests/features/concurrent_mut_fixtures.feature",
    name = "One step mutates harness context and world",
    harness = CountingHarness,
)]
fn one_step_mutates_harness_context_and_world(harness_world: HarnessWorld) {}
