//! Behavioural regression test ensuring mutable fixtures inserted by value can
//! be mutated across step boundaries. A fully macro-driven scenario currently
//! triggers a rustc ICE (tracked in rustc-ice-2025-11-23T23_42_53-46191.txt), so
//! this test exercises the underlying [`StepContext`] plumbing directly until
//! the compiler issue is resolved.

use std::any::Any;

use rstest_bdd::StepContext;

#[derive(Default, Debug, PartialEq, Eq)]
struct CounterWorld {
    count: usize,
}

#[test]
#[expect(
    clippy::expect_used,
    reason = "downcast must succeed when reconstructing the owned fixture"
)]
fn mutable_owned_fixture_round_trip() {
    let world = StepContext::owned_cell(CounterWorld::default());
    let mut ctx = StepContext::default();
    ctx.insert_owned::<CounterWorld>("counter_world", &world);

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
    drop(ctx);
    let final_world = world
        .into_inner()
        .downcast::<CounterWorld>()
        .expect("fixture should downcast to CounterWorld");
    assert_eq!(*final_world, CounterWorld { count: 3 });
}

struct SomeOtherType;

#[test]
fn mutable_owned_fixture_wrong_type_returns_none() {
    let world = StepContext::owned_cell(CounterWorld::default());
    let mut ctx = StepContext::default();
    ctx.insert_owned::<CounterWorld>("counter_world", &world);

    assert!(
        ctx.borrow_ref::<SomeOtherType>("counter_world").is_none(),
        "borrow_ref should return None for a mismatched owned fixture type"
    );
    assert!(
        ctx.borrow_mut::<SomeOtherType>("counter_world").is_none(),
        "borrow_mut should return None for a mismatched owned fixture type"
    );

    drop(ctx);
    let result: Result<Box<SomeOtherType>, Box<dyn Any>> =
        world.into_inner().downcast::<SomeOtherType>();
    assert!(
        result.is_err(),
        "downcast to a wrong type should return Err"
    );
}
