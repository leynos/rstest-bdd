//! Behavioural tests for `execute_step` function.

use rstest::{fixture, rstest};
use rstest_bdd::execution::{ExecutionError, StepExecutionRequest, execute_step};
use rstest_bdd::{StepContext, StepKeyword};

/// Helper enum to represent expected error types for parameterized testing.
enum ExpectedExecutionError {
    SkipWithoutMessage,
    SkipWithMessage(&'static str),
    StepNotFound,
    HandlerFailed,
}

// Creates a `StepExecutionRequest` with standard test defaults.
// Uses index=0, feature_path="test.feature", scenario_name="Test Scenario".
fn make_request(index: usize, keyword: StepKeyword, text: &str) -> StepExecutionRequest<'_> {
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

/// Shared fixture providing a default `StepContext` for test injection.
#[fixture]
fn ctx() -> StepContext<'static> {
    StepContext::default()
}

#[rstest]
fn execute_step_succeeds_without_value(mut ctx: StepContext<'static>) {
    let request = make_request(0, StepKeyword::When, "behavioural");

    let result = execute_step(&request, &mut ctx);

    assert!(result.is_ok(), "execute_step should succeed");
    assert!(
        result.as_ref().is_ok_and(Option::is_none),
        "expected no return value"
    );
}

#[rstest]
fn execute_step_succeeds_with_value(mut ctx: StepContext<'static>) {
    let request = make_request(0, StepKeyword::Given, "returns value");

    let result = execute_step(&request, &mut ctx);

    let Ok(Some(value)) = result else {
        panic!("execute_step should return Ok(Some(value))");
    };
    let Some(downcast) = value.downcast_ref::<i32>() else {
        panic!("value should be i32");
    };
    assert_eq!(*downcast, 42);
}

/// Context fields extracted from an `ExecutionError` for assertion purposes.
struct ErrorContext<'a> {
    index: usize,
    keyword: StepKeyword,
    text: &'a str,
    feature_path: &'a str,
    scenario_name: &'a str,
}

// Asserts common error context fields match request defaults.
fn assert_error_context(
    context: &ErrorContext<'_>,
    expected_keyword: StepKeyword,
    expected_text: &str,
) {
    assert_eq!(context.index, 0, "index should match request");
    assert_eq!(
        context.keyword, expected_keyword,
        "keyword should match request"
    );
    assert_eq!(context.text, expected_text, "text should match request");
    assert_eq!(
        context.feature_path, "test.feature",
        "feature_path should match"
    );
    assert_eq!(
        context.scenario_name, "Test Scenario",
        "scenario_name should match"
    );
}

fn assert_step_not_found(
    error: &ExecutionError,
    expected_keyword: StepKeyword,
    expected_text: &str,
) {
    let ExecutionError::StepNotFound {
        index,
        keyword,
        text,
        feature_path,
        scenario_name,
    } = error
    else {
        panic!("expected StepNotFound error, got: {error:?}");
    };
    let context = ErrorContext {
        index: *index,
        keyword: *keyword,
        text,
        feature_path,
        scenario_name,
    };
    assert_error_context(&context, expected_keyword, expected_text);
    assert!(!error.is_skip(), "StepNotFound should not be a skip");
}

fn assert_handler_failed(
    error: &ExecutionError,
    expected_keyword: StepKeyword,
    expected_text: &str,
) {
    let ExecutionError::HandlerFailed {
        index,
        keyword,
        text,
        error: inner,
        feature_path,
        scenario_name,
    } = error
    else {
        panic!("expected HandlerFailed error, got: {error:?}");
    };
    let context = ErrorContext {
        index: *index,
        keyword: *keyword,
        text,
        feature_path,
        scenario_name,
    };
    assert_error_context(&context, expected_keyword, expected_text);
    assert!(
        std::error::Error::source(error).is_some(),
        "should have source"
    );
    assert!(
        !inner.to_string().is_empty(),
        "inner error should have message"
    );
    assert!(!error.is_skip(), "HandlerFailed should not be a skip");
}

// Validates that the error matches the expected variant and contains correct context.
fn assert_error_matches(
    error: &ExecutionError,
    expected: &ExpectedExecutionError,
    expected_keyword: StepKeyword,
    expected_text: &str,
) {
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
                Some(*msg),
                "skip message should be preserved"
            );
        }
        ExpectedExecutionError::StepNotFound => {
            assert_step_not_found(error, expected_keyword, expected_text);
        }
        ExpectedExecutionError::HandlerFailed => {
            assert_handler_failed(error, expected_keyword, expected_text);
        }
    }
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
    mut ctx: StepContext<'static>,
    #[case] keyword: StepKeyword,
    #[case] text: &str,
    #[case] expected: ExpectedExecutionError,
) {
    let request = make_request(0, keyword, text);

    let result = execute_step(&request, &mut ctx);

    let Err(error) = result else {
        panic!("execute_step should return Err");
    };

    assert_error_matches(&error, &expected, keyword, text);
}

#[rstest]
fn execute_step_returns_missing_fixtures_error(mut ctx: StepContext<'static>) {
    let request = make_request(0, StepKeyword::Then, "needs fixture");

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
    // Create a non-'static context here because we need to insert a local reference.
    // This test cannot use the shared fixture since the fixture reference
    // must outlive the context.
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
