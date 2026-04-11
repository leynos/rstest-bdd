//! Shared support for async semantic behaviour tests.

use std::cell::RefCell;
use std::path::Path;

use regex::Regex;
#[cfg(feature = "diagnostics")]
use serde_json::Value;

pub(crate) const FEATURE_PATH: &str = "tests/features/async_semantic_behaviour.feature";
pub(crate) const SKIP_SCENARIO_NAME: &str = "async skip propagation preserves metadata";
pub(crate) const ERROR_SCENARIO_NAME: &str = "async failure surfaces scenario metadata";

#[derive(Default)]
struct TestState {
    events: Vec<String>,
    cleanup_drops: usize,
}

thread_local! {
    static TEST_STATE: RefCell<TestState> = RefCell::new(TestState::default());
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct SemanticValue(pub(crate) i32);

pub(crate) struct CleanupProbe;

impl Drop for CleanupProbe {
    fn drop(&mut self) {
        TEST_STATE.with(|state| {
            state.borrow_mut().cleanup_drops += 1;
        });
    }
}

pub(crate) fn clear_events() {
    TEST_STATE.with(|state| {
        state.borrow_mut().events.clear();
    });
}

pub(crate) fn push_event(event: impl Into<String>) {
    TEST_STATE.with(|state| {
        state.borrow_mut().events.push(event.into());
    });
}

pub(crate) fn snapshot_events() -> Vec<String> {
    TEST_STATE.with(|state| state.borrow().events.clone())
}

pub(crate) fn reset_cleanup_drops() {
    TEST_STATE.with(|state| {
        state.borrow_mut().cleanup_drops = 0;
    });
}

pub(crate) fn cleanup_drops() -> usize {
    TEST_STATE.with(|state| state.borrow().cleanup_drops)
}

pub(crate) fn assert_feature_path_suffix(actual: &str, expected_suffix: &str) {
    let actual_path = Path::new(actual);
    let expected = Path::new(expected_suffix);
    assert!(
        actual_path.ends_with(expected),
        "feature path should reference {expected_suffix}, got {actual}",
    );
}

pub(crate) fn assert_handler_failure_context(
    message: &str,
    expected_suffix: &str,
    scenario_name: &str,
    step_keyword: &str,
    step_text: &str,
    function_name: &str,
    handler_error: &str,
) {
    let normalized_message = normalize_message(message);
    let pattern = format!(
        r"Step failed at index .*?: .*?{step_keyword}.*?{step_text}.*?{function_name}.*?{handler_error}.*?\(feature: .*?{expected_suffix}, scenario: .*?{scenario_name}\)",
        step_keyword = regex::escape(step_keyword),
        step_text = regex::escape(step_text),
        function_name = regex::escape(function_name),
        handler_error = regex::escape(handler_error),
        expected_suffix = regex::escape(expected_suffix),
        scenario_name = regex::escape(scenario_name),
    );
    let matcher = Regex::new(&pattern)
        .unwrap_or_else(|error| panic!("handler-failure matcher should compile: {error}"));
    assert!(
        matcher.is_match(&normalized_message),
        "panic message should include the handler failure context: {message}",
    );
}

pub(crate) fn scenario_line(scenario_name: &str) -> u32 {
    let feature_path = Path::new(env!("CARGO_MANIFEST_DIR")).join(FEATURE_PATH);
    let feature = std::fs::read_to_string(&feature_path)
        .unwrap_or_else(|error| panic!("feature file should be readable: {error}"));
    feature
        .lines()
        .enumerate()
        .find_map(|(index, line)| {
            parse_scenario_heading(line)
                .filter(|name| *name == scenario_name)
                .map(|_| index + 1)
        })
        .and_then(|line| u32::try_from(line).ok())
        .unwrap_or_else(|| panic!("scenario '{scenario_name}' should exist in {FEATURE_PATH}"))
}

fn parse_scenario_heading(line: &str) -> Option<&str> {
    let trimmed = line.trim_start();
    trimmed
        .strip_prefix("Scenario: ")
        .or_else(|| trimmed.strip_prefix("Scenario Outline: "))
}

fn normalize_message(message: &str) -> String {
    message
        .replace('\\', "/")
        .replace(['\u{2068}', '\u{2069}'], "")
}

#[cfg(feature = "diagnostics")]
pub(crate) fn assert_bypassed_step_recorded(
    scenario_name: &str,
    scenario_line: u32,
    step_pattern: &str,
    reason: &str,
) {
    let dump = match rstest_bdd::dump_registry() {
        Ok(dump) => dump,
        Err(error) => panic!("registry dump should serialize: {error}"),
    };
    let parsed: Value = match serde_json::from_str(&dump) {
        Ok(parsed) => parsed,
        Err(error) => panic!("registry dump should be valid JSON: {error}"),
    };
    let bypassed_steps = parsed
        .get("bypassed_steps")
        .and_then(Value::as_array)
        .unwrap_or_else(|| panic!("registry dump should include bypassed_steps"));
    assert!(
        bypassed_steps.iter().any(|entry| {
            entry.get("scenario_name") == Some(&Value::String(scenario_name.into()))
                && entry.get("scenario_line").and_then(Value::as_u64)
                    == Some(u64::from(scenario_line))
                && entry.get("pattern") == Some(&Value::String(step_pattern.into()))
                && entry
                    .get("reason")
                    .and_then(Value::as_str)
                    .is_some_and(|message| message.contains(reason))
        }),
        "expected a bypassed-step record for scenario '{scenario_name}' and pattern '{step_pattern}'",
    );
}
