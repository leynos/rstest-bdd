//! End-to-end behaviour tests for underscore-prefixed implicit fixture keys.

use rstest::fixture;
use rstest_bdd_macros::{given, scenario, then, when};

#[derive(Default)]
struct StreamingState {
    parsed_events: usize,
}

#[rstest_bdd_test_macros::allow_fixture_expansion_lints]
#[fixture]
fn world() -> StreamingState { StreamingState::default() }

#[rstest_bdd_test_macros::allow_fixture_expansion_lints]
#[fixture]
fn _world() -> &'static str { "explicit _world fixture" }

// The step bodies read the injected fixture with an assertion, then hand it to
// a helper that performs the mutation. The read keeps
// `clippy::used_underscore_binding` firing on every toolchain, so the
// `#[expect]` below is fulfilled on all of them: the lint's span moved between
// compilers, and an `#[expect]` that one toolchain does not trigger is itself
// an error. The assertion also proves both steps share one fixture instance.
// `allow` is not an option here, since the workspace denies bare `allow`
// attributes.

#[given("the streaming world is available")]
#[expect(
    clippy::used_underscore_binding,
    reason = "the test proves underscore-prefixed implicit fixture injection can be used directly"
)]
fn world_is_available(_world: &mut StreamingState) {
    assert_eq!(
        _world.parsed_events, 0,
        "a new scenario should start with an untouched world fixture"
    );
    record_first_event(_world);
}

#[when("the parser runs once more")]
#[expect(
    clippy::used_underscore_binding,
    reason = "the test proves underscore-prefixed implicit fixture injection can be used directly"
)]
fn parser_runs_again(_world: &mut StreamingState) {
    assert_eq!(
        _world.parsed_events, 1,
        "the given step should have run against this same fixture"
    );
    record_another_event(_world);
}

/// Mark the world as having handled its first event.
fn record_first_event(state: &mut StreamingState) { state.parsed_events = 1; }

/// Advance the world by one event.
fn record_another_event(state: &mut StreamingState) { state.parsed_events += 1; }

#[then("implicit underscore fixture lookup uses the world fixture")]
#[expect(
    clippy::used_underscore_binding,
    reason = "the test proves underscore-prefixed implicit fixture injection can be used directly"
)]
fn implicit_lookup_uses_world_fixture(_world: &StreamingState) {
    assert_eq!(_world.parsed_events, 2);
}

#[then("explicit from keeps the underscore-prefixed fixture key")]
fn explicit_from_keeps_underscore_fixture_key(#[from(_world)] fixture_name: &'static str) {
    assert_eq!(fixture_name, "explicit _world fixture");
}

#[scenario(path = "tests/features/underscore_fixture.feature")]
fn implicit_underscore_fixture_keys_are_normalized(
    _world: StreamingState,
    #[from(_world)] explicit_fixture_name: &'static str,
) {
    let _ = explicit_fixture_name;
}
