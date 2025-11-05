//! Behavioural coverage for scenario skipping semantics.

use std::path::Path;

use rstest::fixture;
use rstest_bdd as bdd;
use rstest_bdd_macros::{given, scenario, then};
use serial_test::serial;

use bdd::reporting::{
    self, drain as drain_reports, record as record_scenario, ScenarioRecord, ScenarioStatus,
    SkippedScenario,
};
#[cfg(feature = "diagnostics")]
use serde_json::Value;

#[must_use]
struct FailOnSkippedGuard;

impl FailOnSkippedGuard {
    fn enable() -> Self {
        bdd::config::set_fail_on_skipped(true);
        Self
    }

    fn disable() -> Self {
        bdd::config::set_fail_on_skipped(false);
        Self
    }
}

impl Drop for FailOnSkippedGuard {
    // Clearing the override re-exposes the RSTEST_BDD_FAIL_ON_SKIPPED variable.
    // Tests using this guard must be marked #[serial] to avoid races.
    fn drop(&mut self) {
        bdd::config::clear_fail_on_skipped_override();
    }
}

#[fixture]
fn fail_on_enabled() -> FailOnSkippedGuard {
    FailOnSkippedGuard::enable()
}

#[fixture]
fn fail_on_disabled() -> FailOnSkippedGuard {
    FailOnSkippedGuard::disable()
}

fn assert_feature_path_suffix(actual: &str, expected_suffix: &str) {
    let actual_path = Path::new(actual);
    let expected = Path::new(expected_suffix);
    assert!(
        actual_path.ends_with(expected),
        "feature path should reference {expected_suffix}",
    );
}

#[given("a scenario will be skipped")]
fn skip_scenario() {
    bdd::skip!("skip requested for coverage");
}

#[given("a scenario will skip without a message")]
fn skip_scenario_without_message() {
    bdd::skip!();
}

#[given("a scenario completes successfully")]
fn scenario_completes_successfully() {}

#[then("a trailing step executes")]
fn trailing_step_should_not_run() {
    panic!("trailing step should not execute after a skip request");
}

#[scenario(path = "tests/features/skip.feature", name = "disallowed skip")]
#[serial]
#[should_panic(expected = "Scenario skipped with fail_on_skipped enabled")]
fn disallowed_skip(fail_on_enabled: FailOnSkippedGuard) {
    let _ = &fail_on_enabled;
    unreachable!("scenario should have failed before executing the body");
}

#[scenario(path = "tests/features/skip.feature", name = "allowed skip")]
#[serial]
fn allowed_skip(fail_on_enabled: FailOnSkippedGuard) {
    let _ = &fail_on_enabled;
    panic!("scenario body should not execute when skip is allowed");
}

#[scenario(
    path = "tests/features/skip.feature",
    name = "allowed skip without message"
)]
#[serial]
fn allowed_skip_without_message(fail_on_enabled: FailOnSkippedGuard) {
    let _ = &fail_on_enabled;
    panic!("scenario body should not execute when skip is allowed without a message");
}

#[scenario(path = "tests/features/skip.feature", name = "skip without fail flag")]
#[serial]
fn skip_without_flag(fail_on_disabled: FailOnSkippedGuard) {
    let _ = &fail_on_disabled;
    panic!("scenario body should not execute when fail_on_skipped is disabled");
}

#[scenario(
    path = "tests/features/skip.feature",
    name = "skip prevents trailing steps"
)]
#[serial]
fn skip_prevents_trailing_steps(fail_on_disabled: FailOnSkippedGuard) {
    let _ = &fail_on_disabled;
    panic!("scenario body should not execute when earlier steps skip");
}

#[scenario(
    path = "tests/features/skip_allowance/feature_tag.feature",
    name = "inherits feature tag"
)]
#[serial]
fn feature_tag_allows_skip(fail_on_enabled: FailOnSkippedGuard) {
    let _ = &fail_on_enabled;
    panic!("scenario body should not execute when feature-level tags allow skipping");
}

#[scenario(
    path = "tests/features/skip_allowance/example_tag.feature",
    name = "example tag ignored"
)]
#[serial]
#[should_panic(expected = "Scenario skipped with fail_on_skipped enabled")]
fn example_tag_does_not_allow_skip(fail_on_enabled: FailOnSkippedGuard, case: String) {
    let _ = case;
    let _ = &fail_on_enabled;
}

#[scenario(path = "tests/features/reporting.feature", name = "scenario passes")]
#[serial]
fn scenario_passes_without_skip() {}

#[test]
#[serial]
fn collector_records_allowed_skip_metadata() {
    let _ = drain_reports();
    let guard = FailOnSkippedGuard::enable();
    allowed_skip();
    drop(guard);
    let records = drain_reports();
    let [record] = records.as_slice() else {
        panic!("expected a single skip record");
    };
    assert_feature_path_suffix(record.feature_path(), "tests/features/skip.feature");
    assert_eq!(record.scenario_name(), "allowed skip");
    match record.status() {
        ScenarioStatus::Skipped(details) => {
            assert_eq!(details.message(), Some("skip requested for coverage"));
            assert!(details.allow_skipped());
            assert!(!details.forced_failure());
        }
        ScenarioStatus::Passed => panic!("expected skipped status"),
    }
}

#[test]
#[serial]
fn collector_marks_forced_failure_skips() {
    let _ = drain_reports();
    let guard = FailOnSkippedGuard::enable();
    let result = std::panic::catch_unwind(disallowed_skip);
    drop(guard);
    assert!(result.is_err(), "disallowed skip should panic");
    let records = drain_reports();
    let [record] = records.as_slice() else {
        panic!("expected a single skip record");
    };
    match record.status() {
        ScenarioStatus::Skipped(details) => {
            assert_eq!(details.message(), Some("skip requested for coverage"));
            assert!(!details.allow_skipped());
            assert!(details.forced_failure());
        }
        ScenarioStatus::Passed => panic!("expected skipped status"),
    }
}

#[test]
#[serial]
fn collector_records_passed_scenarios() {
    let _ = drain_reports();
    scenario_passes_without_skip();
    let records = drain_reports();
    let [record] = records.as_slice() else {
        panic!("expected a single pass record");
    };
    assert_feature_path_suffix(record.feature_path(), "tests/features/reporting.feature");
    assert_eq!(record.scenario_name(), "scenario passes");
    assert!(matches!(record.status(), ScenarioStatus::Passed));
}

#[test]
#[serial]
fn collector_records_skips_without_message() {
    let _ = drain_reports();
    let guard = FailOnSkippedGuard::enable();
    allowed_skip_without_message();
    drop(guard);
    let records = drain_reports();
    let [record] = records.as_slice() else {
        panic!("expected a single skip record without message");
    };
    match record.status() {
        ScenarioStatus::Skipped(details) => {
            assert_eq!(details.message(), None);
            assert!(details.allow_skipped());
            assert!(!details.forced_failure());
        }
        ScenarioStatus::Passed => panic!("expected skipped status"),
    }
}

#[cfg(feature = "diagnostics")]
#[test]
#[serial]
fn json_writer_emits_lowercase_skipped_status() {
    let _ = drain_reports();
    let guard = FailOnSkippedGuard::enable();
    allowed_skip();
    drop(guard);
    let records = reporting::snapshot();
    let json = match reporting::json::to_string(&records) {
        Ok(value) => value,
        Err(error) => panic!("expected JSON serialization to succeed: {error}"),
    };
    let parsed: Value = match serde_json::from_str(&json) {
        Ok(value) => value,
        Err(error) => panic!("expected JSON report to parse: {error}"),
    };
    let scenarios = parsed
        .get("scenarios")
        .and_then(Value::as_array)
        .unwrap_or_else(|| panic!("scenarios array missing"));
    assert_eq!(scenarios.len(), records.len());
    let scenario = scenarios
        .first()
        .unwrap_or_else(|| panic!("scenario entry present"));
    assert_eq!(
        scenario.get("status").and_then(Value::as_str),
        Some("skipped"),
        "status should be lowercase skipped",
    );
    let Some(feature_path) = scenario.get("feature_path").and_then(Value::as_str) else {
        panic!("feature path should surface in JSON output");
    };
    assert_feature_path_suffix(feature_path, "tests/features/skip.feature");
    assert_eq!(
        scenario.get("scenario_name").and_then(Value::as_str),
        Some("allowed skip"),
        "scenario name should surface in JSON output",
    );
    let skip = scenario
        .get("skip")
        .and_then(Value::as_object)
        .unwrap_or_else(|| panic!("skip details present"));
    assert_eq!(
        skip.get("message").and_then(Value::as_str),
        Some("skip requested for coverage"),
        "skip message should round-trip",
    );
    assert_eq!(
        skip.get("allow_skipped").and_then(Value::as_bool),
        Some(true),
        "expected skip to honour allowance flag",
    );
    let _ = drain_reports();
}

#[cfg(feature = "diagnostics")]
#[test]
#[serial]
fn json_writer_omits_absent_skip_messages() {
    let _ = drain_reports();
    let guard = FailOnSkippedGuard::enable();
    allowed_skip_without_message();
    drop(guard);
    let json = reporting::json::snapshot_string()
        .unwrap_or_else(|error| panic!("expected JSON report: {error}"));
    let parsed: Value =
        serde_json::from_str(&json).unwrap_or_else(|error| panic!("expected valid JSON: {error}"));
    let scenario = parsed
        .get("scenarios")
        .and_then(Value::as_array)
        .and_then(|entries| entries.first())
        .unwrap_or_else(|| panic!("scenario entry present"));
    let skip = scenario
        .get("skip")
        .and_then(Value::as_object)
        .unwrap_or_else(|| panic!("skip details present"));
    assert!(skip.get("message").is_none() || skip.get("message") == Some(&Value::Null));
    let _ = drain_reports();
}

#[test]
#[serial]
fn junit_writer_emits_skipped_child_element() {
    let _ = drain_reports();
    let guard = FailOnSkippedGuard::enable();
    allowed_skip();
    drop(guard);
    let records = reporting::snapshot();
    let mut output = String::new();
    if let Err(error) = reporting::junit::write(&mut output, &records) {
        panic!("expected to render JUnit report: {error}");
    }
    assert!(
        output.contains("<skipped message=\"skip requested for coverage\" />"),
        "JUnit output should include skipped element with message",
    );
    assert!(
        output.contains("tests=\"1\" failures=\"0\" skipped=\"1\""),
        "JUnit suite summary should record skip counts",
    );
    let _ = drain_reports();
}

#[test]
#[serial]
fn junit_writer_marks_forced_failure_skips() {
    let _ = drain_reports();
    let guard = FailOnSkippedGuard::enable();
    let _ = std::panic::catch_unwind(disallowed_skip);
    drop(guard);
    let records = reporting::snapshot();
    let mut output = String::new();
    if let Err(error) = reporting::junit::write(&mut output, &records) {
        panic!("expected to render JUnit report: {error}");
    }
    assert!(
        output.contains("<failure type=\"fail_on_skipped\">")
            && output.contains("fail_on_skipped enabled"),
        "forced failure skip should surface as failure in JUnit",
    );
    assert!(
        output.contains("failures=\"1\" skipped=\"1\""),
        "JUnit summary should reflect failure counts",
    );
    let _ = drain_reports();
}

#[test]
#[serial]
fn junit_writer_escapes_special_characters() {
    let _ = drain_reports();
    record_scenario(ScenarioRecord::new(
        "tests/features/<feature>&special",
        "Scenario with <&>\"'",
        ScenarioStatus::Skipped(SkippedScenario::new(
            Some("message with <bad>&chars\u{0007}".into()),
            true,
            false,
        )),
    ));
    let records = reporting::snapshot();
    let mut output = String::new();
    if let Err(error) = reporting::junit::write(&mut output, &records) {
        panic!("expected to render JUnit report: {error}");
    }
    assert!(output.contains("Scenario with &lt;&amp;&gt;&quot;&apos;"));
    assert!(output.contains("tests/features/&lt;feature&gt;&amp;special"));
    assert!(output.contains("message with &lt;bad&gt;&amp;chars"));
    assert!(output.contains("&#xFFFD;"));
    let _ = drain_reports();
}
