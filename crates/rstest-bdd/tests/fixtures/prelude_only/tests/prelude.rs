//! A complete scenario written through `rstest_bdd::prelude` alone.
//!
//! Nothing here is imported from `rstest-bdd-macros`: the glob supplies
//! `#[given]`, `#[when]`, `#[then]`, `#[scenario]`, the `ScenarioState` derive
//! *and* the trait it implements, and `Slot`. That trait/derive pair is the
//! reason the fixture exists in a crate of its own: the macro resolves the
//! runtime path from the consuming manifest through `proc_macro_crate`, and
//! only a separate manifest exercises the case users are in.
//!
//! `rstest`'s `#[fixture]` is the one name imported from outside the facade.
//! It has to be: `rstest_macros` requires `rstest` in the consuming manifest
//! and panics during expansion otherwise, so re-exporting it would only make
//! `rstest` a public dependency without removing the entry.

use rstest::fixture;
use rstest_bdd::prelude::*;

/// Scenario state shared by the steps.
#[derive(Debug, Default, ScenarioState)]
struct Counter {
    /// The running total, filled by the first step.
    value: Slot<u32>,
}

// The fixture expansion wraps this expression body in a redundant block, which
// `unused_braces` reports at the braces below. The workspace's examples silence
// that with `rstest_bdd_test_macros::allow_fixture_expansion_lints`, which this
// manifest deliberately does not name, so the allowance is written out here. It
// is the only lint this fixture needs to touch.
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