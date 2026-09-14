//! JSON diagnostic-report assertions for skipped scenarios.

#![cfg(feature = "diagnostics")]

use rstest_bdd::reporting::{self, drain as drain_reports};
use serde_json::Value;
use serial_test::serial;

use super::{
    FailOnSkippedGuard,
    allowed_skip,
    allowed_skip_without_message,
    assert_feature_path_suffix,
};

#[test]
#[serial(skip_reporting)]
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
    let Some(scenarios) = parsed.get("scenarios").and_then(Value::as_array) else {
        panic!("scenarios array missing");
    };
    assert_eq!(scenarios.len(), records.len());
    let Some(scenario) = scenarios.first() else {
        panic!("scenario entry present");
    };
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
    let Some(skip) = scenario.get("skip").and_then(Value::as_object) else {
        panic!("skip details present");
    };
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

#[test]
#[serial(skip_reporting)]
fn json_writer_omits_absent_skip_messages() {
    let _ = drain_reports();
    let guard = FailOnSkippedGuard::enable();
    allowed_skip_without_message();
    drop(guard);
    let json = match reporting::json::snapshot_string() {
        Ok(json) => json,
        Err(error) => panic!("expected JSON report: {error}"),
    };
    let parsed: Value = match serde_json::from_str(&json) {
        Ok(parsed) => parsed,
        Err(error) => panic!("expected valid JSON: {error}"),
    };
    let Some(scenario) = parsed
        .get("scenarios")
        .and_then(Value::as_array)
        .and_then(|entries| entries.first())
    else {
        panic!("scenario entry present");
    };
    let Some(skip) = scenario.get("skip").and_then(Value::as_object) else {
        panic!("skip details present");
    };
    assert!(skip.get("message").is_none() || skip.get("message") == Some(&Value::Null));
    let _ = drain_reports();
}
