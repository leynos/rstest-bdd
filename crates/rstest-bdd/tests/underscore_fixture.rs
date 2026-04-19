//! End-to-end behaviour tests for underscore-prefixed implicit fixture keys.

use rstest::fixture;
use rstest_bdd_macros::{given, scenario, then, when};

#[derive(Default)]
struct StreamingState {
    parsed_events: usize,
}

#[fixture]
fn world() -> StreamingState {
    StreamingState::default()
}

#[fixture]
fn _world() -> &'static str {
    "explicit _world fixture"
}

#[given("the streaming world is available")]
fn world_is_available(_world: &mut StreamingState) {
    _world.parsed_events = 1;
}

#[when("the parser runs once more")]
fn parser_runs_again(_world: &mut StreamingState) {
    _world.parsed_events += 1;
}

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
