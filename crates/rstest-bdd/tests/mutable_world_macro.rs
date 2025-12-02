#![cfg(test)]
//! Macro-driven coverage for mutable world fixtures.
//!
//! The `#[scenario]` runner now stores owned fixtures mutably so step functions
//! may declare `&mut Fixture` parameters. A rustc internal compiler error (ICE)
//! has affected some nightly compilers when expanding the full macro path, so
//! the real scenario below is gated behind the `mutable_world_macro` feature
//! until the upstream fix lands. The direct `StepContext` regression test
//! remains in
//! `mutable_fixture.rs`.
//!
//! Enable this file's main test with:
//!
//! ```bash
//! cargo test -p rstest-bdd --features mutable_world_macro -- tests::macro_world::mutable_world
//! ```
//!
//! and remove the gate once the compiler bug is resolved.
//!
//! Tracking: see `docs/known-issues.md#rustc-ice-with-mutable-world-macro`.

#[cfg(feature = "mutable_world_macro")]
mod macro_world {
    use rstest::fixture;
    use rstest_bdd_macros::{given, scenario, then, when};

    #[derive(Default, Debug, PartialEq, Eq)]
    struct CounterWorld {
        count: usize,
    }

    #[fixture]
    fn world() -> CounterWorld {
        CounterWorld::default()
    }

    #[given("the world starts at {value}")]
    fn starts_at(world: &mut CounterWorld, value: usize) {
        world.count = value;
    }

    #[when("the world increments")]
    fn increments(world: &mut CounterWorld) {
        world.count += 1;
    }

    #[then("the world equals {expected}")]
    fn equals(world: &CounterWorld, expected: usize) {
        assert_eq!(world.count, expected);
    }

    #[scenario(
        path = "tests/features/mutable_world.feature",
        name = "Steps mutate shared state"
    )]
    fn mutable_world(world: CounterWorld) {
        assert_eq!(world.count, 3);
    }
}

#[cfg(not(feature = "mutable_world_macro"))]
#[test]
fn mutable_world_macro_gated() {
    // Guard against the rustc ICE reproduced in `mutable_fixture.rs`. Re-enable
    // the macro-driven test above by compiling with the `mutable_world_macro`
    // feature once the upstream compiler fix lands.
    assert!(!cfg!(feature = "mutable_world_macro"));
}
