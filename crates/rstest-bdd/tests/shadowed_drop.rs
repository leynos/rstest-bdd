//! Behavioural test for fixture guards when the caller shadows `drop`.
//!
//! Generated wrappers release each fixture borrow guard straight after the
//! step call. They must name `::core::mem::drop` explicitly: if they called
//! whatever `drop` is in scope, this module's `drop` would leak the guard and
//! the second mutable borrow of the same fixture would fail.

use rstest::fixture;
use rstest_bdd_macros::{scenario, then, when};

/// Shadows the prelude `drop` by forgetting its argument instead.
fn drop<T>(value: T) { std::mem::forget(value); }

#[derive(Default)]
struct Counter {
    value: u32,
}

#[rstest_bdd_test_macros::allow_fixture_expansion_lints]
#[fixture]
fn counter() -> Counter { Counter::default() }

#[when("the counter is incremented")]
fn increment(counter: &mut Counter) { counter.value += 1; }

#[then("the counter reads {expected}")]
fn counter_reads(counter: &Counter, expected: u32) {
    assert_eq!(counter.value, expected);
}

#[scenario(path = "tests/features/shadowed_drop.feature")]
fn guards_are_released_despite_a_shadowed_drop(counter: Counter) {
    assert_eq!(counter.value, 2);
    drop(counter);
}
