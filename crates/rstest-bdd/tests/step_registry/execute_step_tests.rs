//! Behavioural tests for `execute_step` function.

use rstest_bdd::execution::{ExecutionError, StepExecutionRequest, execute_step};
use rstest_bdd::{StepContext, StepKeyword};

/// Helper to create a `StepExecutionRequest` for testing.
pub fn make_request(index: usize, keyword: StepKeyword, text: &str) -> StepExecutionRequest<'_> {
    StepExecutionRequest {
        index,
        keyword,
        text,
        docstring: None,
        table: None,
        feature_path: "test.feature",
        scenario_name: "Test Scenario",
    }
}

#[test]
fn execute_step_succeeds_without_value() {
    let request = make_request(0, StepKeyword::When, "behavioural");
    let mut ctx = StepContext::default();

    let result = execute_step(&request, &mut ctx);

    assert!(result.is_ok(), "execute_step should succeed");
    assert!(
        result.as_ref().is_ok_and(Option::is_none),
        "expected no return value"
    );
}

#[test]
#[expect(clippy::expect_used, reason = "test validates downcast succeeds")]
fn execute_step_succeeds_with_value() {
    let request = make_request(0, StepKeyword::Given, "returns value");
    let mut ctx = StepContext::default();

    let result = execute_step(&request, &mut ctx);

    assert!(result.is_ok(), "execute_step should succeed");
    let value = result
        .expect("result should be Ok")
        .expect("should have value");
    let downcast = value.downcast_ref::<i32>().expect("should be i32");
    assert_eq!(*downcast, 42);
}

#[test]
#[expect(
    clippy::expect_used,
    reason = "test uses expect_err to unwrap for assertions"
)]
fn execute_step_skip_without_message_returns_skip_error() {
    let request = make_request(0, StepKeyword::When, "skips without message");
    let mut ctx = StepContext::default();

    let result = execute_step(&request, &mut ctx);

    assert!(result.is_err(), "execute_step should return Err for skip");
    let error = result.expect_err("expected skip signal");
    assert!(error.is_skip(), "error should be a skip signal");
    assert_eq!(
        error.skip_message(),
        None,
        "skip without message should have None"
    );
}

#[test]
#[expect(
    clippy::expect_used,
    reason = "test uses expect_err to unwrap for assertions"
)]
fn execute_step_skip_with_message_returns_skip_error() {
    let request = make_request(0, StepKeyword::Then, "skips with message");
    let mut ctx = StepContext::default();

    let result = execute_step(&request, &mut ctx);

    assert!(result.is_err(), "execute_step should return Err for skip");
    let error = result.expect_err("expected skip signal");
    assert!(error.is_skip(), "error should be a skip signal");
    assert_eq!(
        error.skip_message(),
        Some("test skip reason"),
        "skip message should be preserved"
    );
}

#[test]
#[expect(
    clippy::expect_used,
    reason = "test uses expect_err to unwrap for assertions"
)]
fn execute_step_returns_step_not_found_error() {
    let request = make_request(0, StepKeyword::Given, "nonexistent step pattern");
    let mut ctx = StepContext::default();

    let result = execute_step(&request, &mut ctx);

    assert!(result.is_err(), "execute_step should return Err");
    let error = result.expect_err("expected error");
    assert!(
        matches!(error, ExecutionError::StepNotFound { index: 0, .. }),
        "expected StepNotFound error, got: {error:?}"
    );
    assert!(!error.is_skip(), "StepNotFound should not be a skip");
}

#[test]
#[expect(
    clippy::expect_used,
    reason = "test uses expect_err to unwrap for assertions"
)]
fn execute_step_returns_handler_failed_error() {
    let request = make_request(0, StepKeyword::Given, "fails");
    let mut ctx = StepContext::default();

    let result = execute_step(&request, &mut ctx);

    assert!(result.is_err(), "execute_step should return Err");
    let error = result.expect_err("expected error");
    assert!(
        matches!(error, ExecutionError::HandlerFailed { index: 0, .. }),
        "expected HandlerFailed error, got: {error:?}"
    );
    assert!(!error.is_skip(), "HandlerFailed should not be a skip");
}

#[test]
#[expect(
    clippy::expect_used,
    reason = "test uses expect_err to unwrap for assertions"
)]
fn execute_step_returns_missing_fixtures_error() {
    let request = make_request(0, StepKeyword::Then, "needs fixture");
    let mut ctx = StepContext::default();

    let result = execute_step(&request, &mut ctx);

    assert!(result.is_err(), "execute_step should return Err");
    let error = result.expect_err("expected error");
    assert!(
        matches!(error, ExecutionError::MissingFixtures(_)),
        "expected MissingFixtures error, got: {error:?}"
    );
    assert!(!error.is_skip(), "MissingFixtures should not be a skip");
    // Verify the missing fixture is correctly identified
    if let ExecutionError::MissingFixtures(details) = &error {
        assert!(
            details.missing.contains(&"missing"),
            "expected 'missing' in missing fixtures list"
        );
    }
}

#[test]
fn execute_step_succeeds_when_fixtures_are_present() {
    let request = make_request(0, StepKeyword::Then, "needs fixture");
    let value = 42u32;
    let mut ctx = StepContext::default();
    ctx.insert("missing", &value);

    // The step wrapper checks for the fixture and returns MissingFixture error if absent,
    // but fixture validation happens first. With the fixture present, the step succeeds.
    let result = execute_step(&request, &mut ctx);

    assert!(
        result.is_ok(),
        "execute_step should succeed when fixtures present"
    );
}
