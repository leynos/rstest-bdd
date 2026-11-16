//! The same scenario written through `rstest_bdd_macros` directly.
//!
//! This is the import shape the four examples used before they migrated to
//! `rstest_bdd::prelude`. Keeping one compiled consumer of it means the macro
//! crate's own entry point stays covered by a real downstream crate once no
//! example names it any more, and it gives the prelude fixture's manifest
//! assertion a negative control: here the macros crate *is* declared, and the
//! same reading must find it.

use rstest::fixture;
use rstest_bdd::Slot;
use rstest_bdd_macros::{ScenarioState, given, scenario, then, when};

/// Scenario state shared by the steps.
#[derive(Debug, Default, ScenarioState)]
struct Counter {
    /// The running total, filled by the first step.
    value: Slot<u32>,
}

// The fixture expansion wraps this expression body in a redundant block, which
// `unused_braces` reports at the braces below. The examples silence that with
// `rstest_bdd_test_macros::allow_fixture_expansion_lints`; this fixture does
// not name that crate, so the allowance is written out.
#[allow(
    unused_braces,
    reason = "fixture macro expansion adds a redundant block around expression bodies"
)]
#[fixture]
fn counter() -> Counter { Counter::default() }

#[given("a counter starting at {start:u32}")]
fn counter_starting_at(counter: &Counter, start: u32) { counter.value.set(start); }

#[when("I add {amount:u32} to the counter")]
fn add_to_counter(counter: &Counter, amount: u32) {
    counter
        .value
        .set(counter.value.get().map_or(amount, |total| total + amount));
}

#[then("the counter reads {expected:u32}")]
fn counter_reads(counter: &Counter, expected: u32) {
    assert_eq!(counter.value.get(), Some(expected));
}

#[scenario(
    path = "tests/features/counter.feature",
    name = "Counting up from zero"
)]
fn counting_up_from_zero(#[from(counter)] _counter: Counter) {}