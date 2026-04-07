//! End-to-end behavioural tests for Result-returning fixture injection.
//!
//! Verifies that `#[scenario]` can accept fixture parameters typed as
//! `Result<T, E>`, automatically unwrap them with `?`, and inject the
//! inner `T` into step functions via `StepContext`.

use rstest::fixture;
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
    fn try_new() -> Result<Self, String> { Ok(Self { value: 42 }) }

    fn try_new_failing() -> Result<Self, String> {
        Err("fixture initialisation failed".to_string())
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

#[test]
#[serial]
fn result_fixture_success_records_pass() {
    let _ = drain_reports();
    let result = result_fixture_success();
    assert!(
        result.is_ok(),
        "scenario with successful Result fixture should return Ok"
    );
    let records = drain_reports();
    let passed_count = records
        .iter()
        .filter(|r| matches!(r.status(), ScenarioStatus::Passed))
        .count();
    assert_eq!(
        1, passed_count,
        "expected exactly one Passed record for successful Result fixture scenario"
    );
}

#[test]
#[serial]
fn result_fixture_error_propagates() {
    let _ = drain_reports();
    let result = result_fixture_error();
    let Err(err) = result else {
        panic!("scenario with failing Result fixture should return Err");
    };
    assert!(
        err.contains("fixture initialisation failed"),
        "error should contain fixture failure message, got: {err}"
    );
}
