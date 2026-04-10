//! End-to-end behavioural tests for Result-returning fixture injection.
//!
//! Verifies that `#[scenario]` can accept fixture parameters typed as
//! `Result<T, E>`, automatically unwrap them with `?`, and inject the
//! inner `T` into step functions via `StepContext`.

use rstest::fixture;
use rstest_bdd::StepResult;
use rstest_bdd::reporting::{ScenarioStatus, drain as drain_reports};
use rstest_bdd_macros::{given, scenario, then, when};
use serial_test::serial;

/// A simple world type initialised through a fallible constructor.
#[derive(Default)]
struct ResultWorld {
    value: u32,
}

impl ResultWorld {
    #[expect(
        clippy::unnecessary_wraps,
        reason = "returns Result to exercise the Result-unwrapping fixture codegen path"
    )]
    fn try_new() -> Result<Self, String> {
        Ok(Self { value: 42 })
    }

    fn try_new_failing() -> Result<Self, String> {
        Err("fixture initialization failed".to_string())
    }
}

/// Fixture that returns `Result<ResultWorld, String>`.
#[fixture]
fn world() -> Result<ResultWorld, String> {
    ResultWorld::try_new()
}

/// Fixture that always fails, for testing error propagation.
#[fixture]
fn failing_world() -> Result<ResultWorld, String> {
    ResultWorld::try_new_failing()
}

#[given("a world initialised from a Result fixture")]
fn given_world(world: &ResultWorld) {
    assert_eq!(world.value, 42, "world should be initialised with value 42");
}

#[when("the world is mutated")]
fn when_mutated(world: &mut ResultWorld) {
    world.value += 1;
}

#[then("the world reflects the mutation")]
fn then_mutated(world: &ResultWorld) {
    assert_eq!(world.value, 43, "world value should be 43 after mutation");
}

#[scenario(
    path = "tests/features/result_fixture.feature",
    name = "successful fixture initialisation"
)]
#[serial]
fn result_fixture_success(world: Result<ResultWorld, String>) -> Result<(), String> {
    Ok(())
}

#[scenario(
    path = "tests/features/result_fixture.feature",
    name = "failing fixture initialisation"
)]
#[serial]
#[ignore = "exercised by result_fixture_error_propagates"]
fn result_fixture_error(
    #[from(failing_world)] world: Result<ResultWorld, String>,
) -> Result<(), String> {
    Ok(())
}

fn assert_scenario_passes<E: std::fmt::Debug>(run: impl FnOnce() -> Result<(), E>, label: &str) {
    let _ = drain_reports();
    let result = run();
    assert!(
        result.is_ok(),
        "scenario '{label}' should return Ok, got {result:?}"
    );
    let records = drain_reports();
    let passed_count = records
        .iter()
        .filter(|r| matches!(r.status(), ScenarioStatus::Passed))
        .count();
    assert_eq!(
        1, passed_count,
        "expected exactly one Passed record for '{label}'"
    );
}

fn assert_scenario_error_propagates<E: std::fmt::Display>(
    run: impl FnOnce() -> Result<(), E>,
    expected_fragment: &str,
    label: &str,
) {
    let _ = drain_reports();
    let result = run();
    let Err(err) = result else {
        panic!("scenario '{label}' should return Err");
    };
    assert!(
        err.to_string().contains(expected_fragment),
        "error for '{label}' should contain '{expected_fragment}', got: {err}"
    );
    let records = drain_reports();
    assert!(
        records
            .iter()
            .all(|r| !matches!(r.status(), ScenarioStatus::Passed)),
        "failing scenario '{label}' should not record Passed status"
    );
}

#[test]
#[serial]
fn result_fixture_success_records_pass() {
    assert_scenario_passes(result_fixture_success, "successful Result fixture");
}

#[test]
#[serial]
fn result_fixture_error_propagates() {
    assert_scenario_error_propagates(
        result_fixture_error,
        "fixture initialization failed",
        "failing Result fixture",
    );
}

// -- StepResult<T, E> fixture tests ---

/// Fixture that returns `StepResult<ResultWorld, String>`.
#[fixture]
fn step_result_world() -> StepResult<ResultWorld, String> {
    ResultWorld::try_new()
}

/// Fixture that always fails, for testing `StepResult` error propagation.
#[fixture]
fn failing_step_result_world() -> StepResult<ResultWorld, String> {
    Err("step-result fixture initialization failed".to_string())
}

#[given("a world initialised from a StepResult fixture")]
fn given_step_result_world(step_result_world: &ResultWorld) {
    assert_eq!(
        step_result_world.value, 42,
        "world from StepResult fixture should be initialised with value 42"
    );
}

#[when("the StepResult world is mutated")]
fn when_step_result_mutated(step_result_world: &mut ResultWorld) {
    step_result_world.value += 10;
}

#[then("the StepResult world reflects the mutation")]
fn then_step_result_mutated(step_result_world: &ResultWorld) {
    assert_eq!(
        step_result_world.value, 52,
        "world value should be 52 after mutation"
    );
}

#[scenario(
    path = "tests/features/result_fixture.feature",
    name = "StepResult fixture success"
)]
#[serial]
fn step_result_fixture_success(
    step_result_world: StepResult<ResultWorld, String>,
) -> StepResult<(), String> {
    Ok(())
}

#[scenario(
    path = "tests/features/result_fixture.feature",
    name = "StepResult fixture error"
)]
#[serial]
#[ignore = "exercised by step_result_fixture_error_propagates"]
fn step_result_fixture_error(
    #[from(failing_step_result_world)] step_result_world: StepResult<ResultWorld, String>,
) -> StepResult<(), String> {
    Ok(())
}

#[test]
#[serial]
fn step_result_fixture_success_records_pass() {
    assert_scenario_passes(step_result_fixture_success, "successful StepResult fixture");
}

#[test]
#[serial]
fn step_result_fixture_error_propagates() {
    assert_scenario_error_propagates(
        step_result_fixture_error,
        "step-result fixture initialization failed",
        "failing StepResult fixture",
    );
}
