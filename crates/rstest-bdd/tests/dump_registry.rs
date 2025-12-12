#![cfg(feature = "diagnostics")]
//! Unit tests for registry dumping.

use rstest_bdd::{
    StepContext, StepExecution, StepKeyword, dump_registry, find_step,
    reporting::{self, ScenarioRecord, ScenarioStatus, SkippedScenario},
    step,
};
use serde_json::Value;

mod common;
use common::noop_wrapper;

step!(StepKeyword::Given, "dump used", noop_wrapper, &[]);
step!(StepKeyword::Given, "dump unused", noop_wrapper, &[]);

fn execute_and_validate_step(keyword: StepKeyword, pattern: &str) {
    let runner = find_step(keyword, pattern.into()).unwrap_or_else(|| panic!("step not found"));
    let mut ctx = StepContext::default();
    match runner(&mut ctx, pattern, None, None) {
        Ok(StepExecution::Continue { .. }) => {}
        Ok(StepExecution::Skipped { .. }) => panic!("step unexpectedly skipped"),
        Err(e) => panic!("execution failed: {e}"),
    }
}

fn validate_skipped_scenario(scenarios: &[Value]) {
    let skipped = scenarios
        .iter()
        .find(|entry| entry.get("status") == Some(&Value::String("skipped".into())))
        .unwrap_or_else(|| panic!("skipped scenario present"));
    assert_eq!(
        skipped.get("message").and_then(Value::as_str),
        Some("reason"),
        "skip message should be preserved",
    );
    assert_eq!(
        skipped.get("allow_skipped").and_then(Value::as_bool),
        Some(true),
        "skip allowance flag should surface",
    );
    assert_eq!(
        skipped.get("forced_failure").and_then(Value::as_bool),
        Some(false),
        "skip should not record forced failure flag",
    );
}

fn validate_passed_scenario(scenarios: &[Value]) {
    let passing = scenarios
        .iter()
        .find(|entry| entry.get("status") == Some(&Value::String("passed".into())))
        .unwrap_or_else(|| panic!("passed scenario present"));
    assert!(
        passing.get("message").is_none() || passing.get("message") == Some(&Value::Null),
        "passed scenarios should not include a skip message",
    );
    assert_eq!(
        passing.get("allow_skipped").and_then(Value::as_bool),
        Some(false),
        "passed scenarios should not advertise skip allowance",
    );
    assert_eq!(
        passing.get("forced_failure").and_then(Value::as_bool),
        Some(false),
        "passed scenarios should not force failures",
    );
}

#[test]
fn reports_usage_flags() {
    let _ = reporting::drain();
    reporting::record(ScenarioRecord::new(
        "tests/features/dump.feature",
        "skipped entry",
        ScenarioStatus::Skipped(SkippedScenario::new(Some("reason".into()), true, false)),
    ));
    reporting::record(ScenarioRecord::new(
        "tests/features/dump.feature",
        "passing entry",
        ScenarioStatus::Passed,
    ));

    execute_and_validate_step(StepKeyword::Given, "dump used");

    let json = dump_registry().unwrap_or_else(|e| panic!("dump registry: {e}"));
    let parsed: Value = serde_json::from_str(&json).unwrap_or_else(|e| panic!("valid json: {e}"));
    let steps = parsed
        .get("steps")
        .and_then(Value::as_array)
        .unwrap_or_else(|| panic!("steps array"));
    assert!(
        steps
            .iter()
            .any(|s| s["pattern"] == "dump used" && s["used"].as_bool() == Some(true)),
        "expected 'dump used' to be marked used"
    );
    assert!(
        steps
            .iter()
            .any(|s| s["pattern"] == "dump unused" && s["used"].as_bool() == Some(false)),
        "expected 'dump unused' to be marked unused"
    );

    let scenarios = parsed
        .get("scenarios")
        .and_then(Value::as_array)
        .unwrap_or_else(|| panic!("scenarios array"));
    assert!(scenarios.iter().all(|entry| entry.get("status").is_some()));
    validate_skipped_scenario(scenarios);
    validate_passed_scenario(scenarios);
    let _ = reporting::drain();
}
