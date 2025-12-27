//! Compile-pass fixture for `scenarios!` with `fixtures = [...]` argument.
//!
//! Verifies that:
//! - The `fixtures = [name: Type, ...]` syntax parses correctly.
//! - The generated test function includes fixture parameters.
//! - The `#[expect(unused_variables)]` attribute is applied to suppress
//!   lint warnings for fixture variables consumed via StepContext.

use rstest::fixture;
use rstest_bdd::StepContext;
use rstest_bdd_macros::{given, scenarios, then, when};
use std::cell::RefCell;

/// A simple counter world fixture for testing.
#[derive(Default)]
struct CounterWorld {
    count: u32,
}

/// An rstest fixture that provides a counter world.
#[fixture]
fn counter_world() -> RefCell<CounterWorld> {
    RefCell::new(CounterWorld::default())
}

#[given("a counter fixture")]
fn a_counter_fixture(counter_world: &RefCell<CounterWorld>) {
    assert_eq!(counter_world.borrow().count, 0);
}

#[when("the counter is incremented")]
fn the_counter_is_incremented(counter_world: &RefCell<CounterWorld>) {
    counter_world.borrow_mut().count += 1;
}

#[then("the counter equals 1")]
fn the_counter_equals_1(counter_world: &RefCell<CounterWorld>) {
    assert_eq!(counter_world.borrow().count, 1);
}

scenarios!(
    "tests/fixtures_macros",
    fixtures = [counter_world: RefCell<CounterWorld>]
);

fn main() {}
