//! Behavioural regression test ensuring mutable fixtures inserted by value can
//! be mutated across step boundaries. A fully macro-driven scenario currently
//! triggers a rustc ICE (tracked in rustc-ice-2025-11-23T23_42_53-46191.txt), so
//! this test exercises the underlying [`StepContext`] plumbing directly until
//! the compiler issue is resolved.

use std::cell::RefCell;

use rstest_bdd::StepContext;

#[derive(Default, Debug, PartialEq, Eq)]
struct CounterWorld {
    count: usize,
}

#[test]
fn mutable_owned_fixture_round_trip() {
    let world = RefCell::new(Box::new(CounterWorld::default()));
    let mut ctx = StepContext::default();
    ctx.insert_owned("counter_world", &world);

    // Given the world starts at 2
    {
        let Some(mut guard) = ctx.borrow_mut::<CounterWorld>("counter_world") else {
            panic!("fixture should exist");
        };
        guard.value_mut().count = 2;
    }

    // When the world increments
    {
        let Some(mut guard) = ctx.borrow_mut::<CounterWorld>("counter_world") else {
            panic!("fixture should exist");
        };
        guard.value_mut().count += 1;
    }

    // Then the scenario body receives the mutated fixture.
    let final_world = world.into_inner();
    assert_eq!(*final_world, CounterWorld { count: 3 });
}
