//! Behavioural tests for `execute_step` function.

use rstest::rstest;
use rstest_bdd::execution::{ExecutionError, StepExecutionRequest, execute_step};
use rstest_bdd::{StepContext, StepKeyword};

/// Helper enum to represent expected error types for parameterized testing.
enum ExpectedExecutionError {
    SkipWithoutMessage,
    SkipWithMessage(&'static str),
    StepNotFound,
    HandlerFailed,
}

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
fn execute_step_succeeds_with_value() {
    let request = make_request(0, StepKeyword::Given, "returns value");
    let mut ctx = StepContext::default();

    let result = execute_step(&request, &mut ctx);

    let Ok(Some(value)) = result else {
        panic!("execute_step should return Ok(Some(value))");
    };
    let Some(downcast) = value.downcast_ref::<i32>() else {
        panic!("value should be i32");
    };
    assert_eq!(*downcast, 42);
}

#[rstest]
#[case(
    StepKeyword::When,
    "skips without message",
    ExpectedExecutionError::SkipWithoutMessage
)]
#[case(
    StepKeyword::Then,
    "skips with message",
    ExpectedExecutionError::SkipWithMessage("test skip reason")
)]
#[case(
    StepKeyword::Given,
    "nonexistent step pattern",
    ExpectedExecutionError::StepNotFound
)]
#[case(StepKeyword::Given, "fails", ExpectedExecutionError::HandlerFailed)]
fn execute_step_returns_expected_error(
    #[case] keyword: StepKeyword,
    #[case] text: &str,
    #[case] expected: ExpectedExecutionError,
) {
    let request = make_request(0, keyword, text);
    let mut ctx = StepContext::default();

    let result = execute_step(&request, &mut ctx);

    let Err(error) = result else {
        panic!("execute_step should return Err");
    };

    match expected {
        ExpectedExecutionError::SkipWithoutMessage => {
            assert!(error.is_skip(), "error should be a skip signal");
            assert_eq!(
                error.skip_message(),
                None,
                "skip without message should have None"
            );
        }
        ExpectedExecutionError::SkipWithMessage(msg) => {
            assert!(error.is_skip(), "error should be a skip signal");
            assert_eq!(
                error.skip_message(),
                Some(msg),
                "skip message should be preserved"
            );
        }
        ExpectedExecutionError::StepNotFound => {
            let ExecutionError::StepNotFound {
                index,
                keyword: err_keyword,
                text: err_text,
                feature_path,
                scenario_name,
            } = &error
            else {
                panic!("expected StepNotFound error, got: {error:?}");
            };
            assert_eq!(*index, 0, "index should match request");
            assert_eq!(*err_keyword, keyword, "keyword should match request");
            assert_eq!(err_text, text, "text should match request");
            assert_eq!(feature_path, "test.feature", "feature_path should match");
            assert_eq!(scenario_name, "Test Scenario", "scenario_name should match");
            assert!(!error.is_skip(), "StepNotFound should not be a skip");
        }
        ExpectedExecutionError::HandlerFailed => {
            let ExecutionError::HandlerFailed {
                index,
                keyword: err_keyword,
                text: err_text,
                error: inner_error,
                feature_path,
                scenario_name,
            } = &error
            else {
                panic!("expected HandlerFailed error, got: {error:?}");
            };
            assert_eq!(*index, 0, "index should match request");
            assert_eq!(*err_keyword, keyword, "keyword should match request");
            assert_eq!(err_text, text, "text should match request");
            assert_eq!(feature_path, "test.feature", "feature_path should match");
            assert_eq!(scenario_name, "Test Scenario", "scenario_name should match");
            assert!(
                std::error::Error::source(&error).is_some(),
                "HandlerFailed should have a source error"
            );
            // Verify the inner error is accessible and contains relevant info
            let inner_str = inner_error.to_string();
            assert!(
                !inner_str.is_empty(),
                "inner error should have a non-empty message"
            );
            assert!(!error.is_skip(), "HandlerFailed should not be a skip");
        }
    }
}

#[test]
fn execute_step_returns_missing_fixtures_error() {
    let request = make_request(0, StepKeyword::Then, "needs fixture");
    let mut ctx = StepContext::default();

    let result = execute_step(&request, &mut ctx);

    let Err(error) = result else {
        panic!("execute_step should return Err");
    };
    let ExecutionError::MissingFixtures(details) = &error else {
        panic!("expected MissingFixtures error, got: {error:?}");
    };
    assert!(!error.is_skip(), "MissingFixtures should not be a skip");
    assert!(
        details.missing.contains(&"missing"),
        "expected 'missing' in missing fixtures list"
    );
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
