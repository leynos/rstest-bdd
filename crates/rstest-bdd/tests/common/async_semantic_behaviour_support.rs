//! Shared support for async semantic behaviour tests.

use std::cell::RefCell;
use std::path::Path;

use regex::Regex;
#[cfg(feature = "diagnostics")]
use serde_json::Value;

/// Relative path from `CARGO_MANIFEST_DIR` to the async semantic behaviour feature file.
pub(crate) const FEATURE_PATH: &str = "tests/features/async_semantic_behaviour.feature";
/// Canonical name of the skip-propagation scenario in the feature file.
pub(crate) const SKIP_SCENARIO_NAME: &str = "async skip propagation preserves metadata";
/// Canonical name of the error-propagation scenario in the feature file.
pub(crate) const ERROR_SCENARIO_NAME: &str = "async failure surfaces scenario metadata";

#[derive(Default)]
struct TestState {
    events: Vec<String>,
    cleanup_drops: usize,
}

impl TestState {
    const fn new() -> Self {
        Self {
            events: Vec::new(),
            cleanup_drops: 0,
        }
    }
}

thread_local! {
    static TEST_STATE: RefCell<TestState> = const { RefCell::new(TestState::new()) };
}

/// Newtype for an integer fixture value used to verify that async step handlers
/// can return a value that is injected as a fixture into the next step.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct SemanticValue(pub(crate) i32);

/// Marker struct whose [`Drop`] implementation increments the per-thread
/// `cleanup_drops` counter, allowing tests to assert that fixtures are
/// dropped exactly once on both success and failure paths.
pub(crate) struct CleanupProbe;

impl Drop for CleanupProbe {
    fn drop(&mut self) {
        TEST_STATE.with(|state| {
            state.borrow_mut().cleanup_drops += 1;
        });
    }
}

/// Identifies the scenario under test for assertion helpers.
#[derive(Clone, Copy)]
pub(crate) struct ScenarioRef<'a> {
    pub(crate) name: &'a str,
    pub(crate) feature_suffix: &'a str,
}

/// Identifies the step that is expected to have failed.
#[derive(Clone, Copy)]
pub(crate) struct StepRef<'a> {
    pub(crate) keyword: &'a str,
    pub(crate) text: &'a str,
    pub(crate) function_name: &'a str,
    pub(crate) handler_error: &'a str,
}

#[cfg(feature = "diagnostics")]
#[derive(Clone, Copy)]
/// Identifies a bypassed step record expected in diagnostics output.
pub(crate) struct BypassedStepQuery<'a> {
    pub(crate) scenario_name: &'a str,
    pub(crate) scenario_line: u32,
    pub(crate) step_pattern: &'a str,
    pub(crate) reason: &'a str,
}

/// Resets the per-thread event log.
///
/// Call at the start of any test that asserts on event ordering.
pub(crate) fn clear_events() {
    TEST_STATE.with(|state| {
        state.borrow_mut().events.clear();
    });
}

/// Appends `event` to the per-thread event log.
///
/// Call from within step handlers to record execution order.
pub(crate) fn push_event(event: impl Into<String>) {
    TEST_STATE.with(|state| {
        state.borrow_mut().events.push(event.into());
    });
}

/// Returns a snapshot of the per-thread event log without clearing it.
pub(crate) fn snapshot_events() -> Vec<String> {
    TEST_STATE.with(|state| state.borrow().events.clone())
}

/// Resets the per-thread [`CleanupProbe`] drop counter to zero.
///
/// Call before the scenario under test so that assertions start from a known state.
pub(crate) fn reset_cleanup_drops() {
    TEST_STATE.with(|state| {
        state.borrow_mut().cleanup_drops = 0;
    });
}

/// Returns the number of times [`CleanupProbe`] has been dropped in this thread.
pub(crate) fn cleanup_drops() -> usize {
    TEST_STATE.with(|state| state.borrow().cleanup_drops)
}

/// Asserts that `actual` ends with `expected_suffix` using [`Path::ends_with`].
///
/// # Panics
///
/// Panics with a descriptive message if `actual` does not end with `expected_suffix`.
pub(crate) fn assert_feature_path_suffix(actual: &str, expected_suffix: &str) {
    let actual_path = Path::new(actual);
    let expected = Path::new(expected_suffix);
    assert!(
        actual_path.ends_with(expected),
        "feature path should reference {expected_suffix}, got {actual}",
    );
}

/// Asserts that `message` contains the expected failure context for a step handler.
///
/// Normalizes `message` (converts backslashes to `/`, strips Unicode directional marks)
/// and verifies it matches a regex built from the supplied [`ScenarioRef`] and [`StepRef`].
///
/// # Panics
///
/// Panics if the regex fails to compile or if `message` does not match.
pub(crate) fn assert_handler_failure_context(
    message: &str,
    scenario: ScenarioRef<'_>,
    step: StepRef<'_>,
) {
    let ScenarioRef {
        name: scenario_name,
        feature_suffix: expected_suffix,
    } = scenario;
    let StepRef {
        keyword: step_keyword,
        text: step_text,
        function_name,
        handler_error,
    } = step;
    let normalized_message = normalize_message(message);
    let pattern = format!(
        r"{step_keyword}.*?{step_text}.*?{function_name}.*?{handler_error}.*?{expected_suffix}.*?{scenario_name}",
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

/// Returns the 1-based line number of `scenario_name` in [`FEATURE_PATH`].
///
/// Reads the feature file relative to `CARGO_MANIFEST_DIR` and scans for
/// a `Scenario:` or `Scenario Outline:` heading matching `scenario_name`.
///
/// # Panics
///
/// Panics if the feature file cannot be read or if no matching scenario is found.
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

/// Asserts that the diagnostics registry contains a bypassed-step record
/// matching all fields of `query`.
///
/// Dumps the registry via `rstest_bdd::dump_registry`, parses the JSON, and
/// searches `bypassed_steps` for an entry matching `scenario_name`,
/// `scenario_line`, `step_pattern`, and a `reason` substring.
///
/// # Panics
///
/// Panics if the dump fails, the JSON is invalid, or no matching entry is found.
#[cfg(feature = "diagnostics")]
pub(crate) fn assert_bypassed_step_recorded(query: BypassedStepQuery<'_>) {
    let BypassedStepQuery {
        scenario_name,
        scenario_line,
        step_pattern,
        reason,
    } = query;
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
