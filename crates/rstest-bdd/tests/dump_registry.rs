//! Unit tests for registry dumping.

use rstest_bdd::{
    StepContext, StepExecution, StepKeyword, dump_registry, find_step, record_bypassed_steps,
    reporting::{self, ScenarioMetadata, ScenarioRecord, ScenarioStatus, SkippedScenario},
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

fn parse_registry_json(json: &str) -> Value {
    serde_json::from_str(json).unwrap_or_else(|e| panic!("valid json: {e}"))
}

fn validate_step_usage_flags(steps: &[Value]) {
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
    assert!(
        steps
            .iter()
            .any(|s| s["pattern"] == "dump unused" && s["bypassed"].as_bool() == Some(true)),
        "expected 'dump unused' to be marked bypassed"
    );
}

fn validate_scenario_metadata(scenarios: &[Value]) {
    let skipped = scenarios
        .iter()
        .find(|entry| entry["scenario_name"] == "skipped entry")
        .unwrap_or_else(|| panic!("skipped scenario present"));
    assert_eq!(skipped["line"].as_u64(), Some(3));
    assert_eq!(
        skipped["tags"]
            .as_array()
            .and_then(|tags| tags.first())
            .and_then(Value::as_str),
        Some("@allow_skipped")
    );
}

fn validate_bypassed_steps_metadata(bypassed_steps: &[Value]) {
    assert!(bypassed_steps.iter().any(|entry| {
        entry.get("pattern") == Some(&Value::String("dump unused".into()))
            && entry
                .get("reason")
                .and_then(Value::as_str)
                .is_some_and(|msg| msg.contains("reason"))
    }));
    let entry = bypassed_steps
        .iter()
        .find(|value| value["pattern"] == "dump unused")
        .unwrap_or_else(|| panic!("bypassed entry present"));
    assert_eq!(entry["scenario_line"].as_u64(), Some(3));
    assert_eq!(
        entry["tags"]
            .as_array()
            .and_then(|tags| tags.first())
            .and_then(Value::as_str),
        Some("@allow_skipped")
    );
}

#[test]
fn reports_usage_flags() {
    let _ = reporting::drain();
    let skipped_metadata = ScenarioMetadata::new(
        "tests/features/dump.feature",
        "skipped entry",
        3,
        vec!["@allow_skipped".into()],
    );
    reporting::record(ScenarioRecord::from_metadata(
        skipped_metadata,
        ScenarioStatus::Skipped(SkippedScenario::new(Some("reason".into()), true, false)),
    ));

    let passing_metadata = ScenarioMetadata::new(
        "tests/features/dump.feature",
        "passing entry",
        4,
        Vec::new(),
    );
    reporting::record(ScenarioRecord::from_metadata(
        passing_metadata,
        ScenarioStatus::Passed,
    ));

    record_bypassed_steps(
        "tests/features/dump.feature",
        "skipped entry",
        3,
        vec!["@allow_skipped".into()],
        Some("reason"),
        [(StepKeyword::Given, "dump unused")],
    );

    execute_and_validate_step(StepKeyword::Given, "dump used");

    let json = dump_registry().unwrap_or_else(|e| panic!("dump registry: {e}"));
    let parsed = parse_registry_json(&json);
    let steps = parsed
        .get("steps")
        .and_then(Value::as_array)
        .unwrap_or_else(|| panic!("steps array"));
    validate_step_usage_flags(steps);

    let scenarios = parsed
        .get("scenarios")
        .and_then(Value::as_array)
        .unwrap_or_else(|| panic!("scenarios array"));
    assert!(scenarios.iter().all(|entry| entry.get("status").is_some()));
    validate_skipped_scenario(scenarios);
    validate_passed_scenario(scenarios);
    validate_scenario_metadata(scenarios);

    let bypassed_steps = parsed
        .get("bypassed_steps")
        .and_then(Value::as_array)
        .unwrap_or_else(|| panic!("bypassed_steps array"));
    validate_bypassed_steps_metadata(bypassed_steps);
    let _ = reporting::drain();
}
