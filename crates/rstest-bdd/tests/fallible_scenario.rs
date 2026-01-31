//! Behavioural coverage for fallible scenario bodies.

use rstest_bdd as bdd;
use rstest_bdd::StepResult;
use rstest_bdd::reporting::{ScenarioStatus, drain as drain_reports};
use rstest_bdd_macros::{given, scenario};
use serial_test::serial;

struct FailOnSkippedGuard;

impl FailOnSkippedGuard {
    fn disable() -> Self {
        bdd::config::set_fail_on_skipped(false);
        Self
    }
}

impl Drop for FailOnSkippedGuard {
    fn drop(&mut self) {
        bdd::config::clear_fail_on_skipped_override();
    }
}

fn fallible_check() -> std::io::Result<()> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");
    std::fs::metadata(path)?;
    Ok(())
}

#[given("a fallible scenario can succeed")]
fn fallible_scenario_succeeds() {}

#[given("a fallible scenario can skip")]
fn fallible_scenario_skips() {
    bdd::skip!("fallible scenario skipped");
}

#[given("a fallible scenario can error")]
fn fallible_scenario_errors() {}

#[scenario(
    path = "tests/features/fallible_scenario.feature",
    name = "Fallible success"
)]
#[serial]
fn fallible_scenario_success() -> Result<(), std::io::Error> {
    fallible_check()?;
    Ok(())
}

#[scenario(
    path = "tests/features/fallible_scenario.feature",
    name = "Fallible skip"
)]
#[serial]
#[ignore = "exercised by fallible_skip_returns_ok_and_records_skip"]
fn fallible_scenario_skip() -> StepResult<(), std::io::Error> {
    fallible_check()?;
    Ok(())
}

#[scenario(
    path = "tests/features/fallible_scenario.feature",
    name = "Fallible error"
)]
#[serial]
#[ignore = "exercised by fallible_error_does_not_record_pass"]
fn fallible_scenario_error() -> Result<(), std::io::Error> {
    Err(std::io::Error::other("forced error"))
}

#[test]
#[serial]
fn fallible_skip_returns_ok_and_records_skip() {
    let _guard = FailOnSkippedGuard::disable();
    let _ = drain_reports();
    let result = fallible_scenario_skip();
    assert!(
        result.is_ok(),
        "expected skipped fallible scenario to return Ok(())"
    );
    let records = drain_reports();
    let [record] = records.as_slice() else {
        panic!("expected a single skip record");
    };
    assert!(matches!(record.status(), ScenarioStatus::Skipped(_)));
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
