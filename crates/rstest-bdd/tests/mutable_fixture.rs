//! Behavioural test ensuring steps can mutate fixtures via &mut references.

use rstest::fixture;
use rstest_bdd_macros::{given, scenario, then, when};

#[derive(Default)]
struct CounterWorld {
    count: usize,
}

#[fixture]
fn counter_world() -> CounterWorld {
    CounterWorld::default()
}

#[given("the world starts at {value}")]
fn seed_world(counter_world: &mut CounterWorld, value: usize) {
    counter_world.count = value;
}

#[when("the world increments")]
fn increment_world(counter_world: &mut CounterWorld) {
    counter_world.count += 1;
}

#[then("the world equals {value}")]
fn assert_world(counter_world: &CounterWorld, value: usize) {
    assert_eq!(counter_world.count, value);
}

#[scenario(path = "tests/features/mutable_world.feature")]
fn mutable_fixture(counter_world: CounterWorld) {
    let _ = counter_world;
}
