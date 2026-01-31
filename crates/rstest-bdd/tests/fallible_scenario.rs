//! Behavioural coverage for fallible scenario bodies.

use rstest_bdd as bdd;
use rstest_bdd::StepResult;
use rstest_bdd::reporting::{ScenarioStatus, drain as drain_reports};
use rstest_bdd_macros::{given, scenario};
use serial_test::serial;

#[given("a fallible scenario succeeds")]
fn fallible_scenario_succeeds() {}

#[given("a fallible scenario fails")]
fn fallible_scenario_fails() {}

#[given("a fallible scenario is skipped")]
fn fallible_scenario_is_skipped() {
    bdd::skip!("fallible scenario requested skip");
}

#[scenario(
    path = "tests/features/fallible_scenario.feature",
    name = "fallible scenario success"
)]
#[serial]
fn fallible_scenario_success() -> Result<(), &'static str> {
    Ok(())
}

#[scenario(
    path = "tests/features/fallible_scenario.feature",
    name = "fallible scenario error"
)]
#[serial]
#[ignore = "exercised by fallible_error_does_not_record_pass"]
fn fallible_scenario_error() -> Result<(), &'static str> {
    Err("fallible scenario returned error")
}

#[scenario(
    path = "tests/features/fallible_scenario.feature",
    name = "fallible scenario skip"
)]
#[serial]
fn fallible_scenario_skip() -> StepResult<(), &'static str> {
    panic!("scenario body should not run after a skip request");
}

#[test]
#[serial]
fn fallible_error_does_not_record_pass() {
    let _ = drain_reports();
    let result = fallible_scenario_error();
    assert!(result.is_err(), "expected fallible scenario to return Err");
    let records = drain_reports();
    assert!(
        records
            .iter()
            .all(|record| !matches!(record.status(), ScenarioStatus::Passed)),
        "fallible scenario error should not record Passed status"
    );
}

#[test]
#[serial]
fn fallible_success_records_pass() {
    let _ = drain_reports();
    let result = fallible_scenario_success();
    assert!(
        result.is_ok(),
        "expected fallible scenario to return Ok(())"
    );
    let records = drain_reports();
    let passed_count = records
        .iter()
        .filter(|record| matches!(record.status(), ScenarioStatus::Passed))
        .count();
    assert_eq!(
        1, passed_count,
        "expected exactly one Passed record for successful fallible scenario"
    );
}
