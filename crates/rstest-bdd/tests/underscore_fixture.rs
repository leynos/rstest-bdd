//! End-to-end behaviour tests for underscore-prefixed fixture parameters.

use rstest::fixture;
use rstest_bdd_macros::{given, scenario, then, when};

const PARAGRAPH_TEI: &str = "<p>Paragraph TEI</p>";

#[derive(Default)]
struct StreamingState {
    xml: Option<&'static str>,
    parsed: bool,
}

#[fixture]
fn state() -> StreamingState {
    StreamingState::default()
}

#[given("the tei_rapporteur Python module is initialised for streaming")]
fn module_initialised(#[from(state)] _state: &mut StreamingState) {}

#[given("the paragraph TEI fixture")]
fn paragraph_fixture(#[from(state)] state: &mut StreamingState) {
    state.xml = Some(PARAGRAPH_TEI);
}

#[when("I stream parse the events")]
fn stream_parse_events(#[from(state)] state: &mut StreamingState) {
    assert_eq!(state.xml, Some(PARAGRAPH_TEI));
    state.parsed = true;
}

#[then("all events decode into msgspec Event instances")]
fn events_decode(#[from(state)] state: &StreamingState) {
    assert_eq!(state.xml, Some(PARAGRAPH_TEI));
    assert!(state.parsed, "events should be parsed before decoding");
}

#[scenario(
    path = "tests/features/python_streaming_parser.feature",
    name = "Events decode into published structs"
)]
fn events_decode_into_structs(state: StreamingState) {
    let _ = state;
}
