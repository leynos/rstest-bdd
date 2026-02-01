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
    name = "fallible scenario async success"
)]
#[serial]
async fn fallible_scenario_async_success() -> Result<(), &'static str> {
    fallible_async_helper().await?;
    Ok(())
}

async fn fallible_async_helper() -> Result<(), &'static str> {
    tokio::task::yield_now().await;
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

#[expect(clippy::panic, reason = "test helper panics for clearer failures")]
fn assert_fallible_success_records_pass(result: Result<(), &'static str>, context: &str) {
    result.unwrap_or_else(|_| panic!("expected {context} scenario to return Ok(())"));
    let records = drain_reports();
    let passed_count = records
        .iter()
        .filter(|record| matches!(record.status(), ScenarioStatus::Passed))
        .count();
    assert_eq!(
        1, passed_count,
        "expected exactly one Passed record for {context} scenario"
    );
}

#[test]
#[serial]
fn fallible_success_records_pass() {
    let _ = drain_reports();
    let result = fallible_scenario_success();
    assert_fallible_success_records_pass(result, "fallible");
}

#[tokio::test]
async fn fallible_async_success_records_pass() {
    #[expect(clippy::panic, reason = "test helper panics for clearer failures")]
    async fn assert_fallible_async_success_records_pass() {
        let join = tokio::task::spawn_blocking(|| {
            serial_test::local_serial_core_with_return("", || {
                let _ = drain_reports();
                let result = crate::fallible_scenario_async_success();
                assert_fallible_success_records_pass(result, "async fallible");
                Ok::<(), &'static str>(())
            })
        })
        .await;

        match join {
            Ok(Ok(())) => {}
            Ok(Err(err)) => panic!("fallible async scenario failed: {err}"),
            Err(err) if err.is_panic() => std::panic::resume_unwind(err.into_panic()),
            Err(err) => panic!("fallible async scenario join failed: {err}"),
        }
    }

    assert_fallible_async_success_records_pass().await;
}
