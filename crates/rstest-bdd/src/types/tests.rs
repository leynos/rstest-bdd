//! Unit tests for shared core types and helper enums.

use super::*;
use crate::localization::{ScopedLocalization, strip_directional_isolates};
use gherkin::StepType;
use rstest::rstest;
use std::str::FromStr;
use unic_langid::langid;

fn kw_from_type(ty: StepType) -> StepKeyword {
    StepKeyword::try_from(ty)
        .map_err(UnsupportedStepType::from)
        .unwrap_or_else(|e| panic!("failed to convert '{ty:?}' into StepKeyword: {e}"))
}

#[rstest]
#[case("Given", StepKeyword::Given)]
#[case("given", StepKeyword::Given)]
#[case("\tThEn\n", StepKeyword::Then)]
#[case("AND", StepKeyword::And)]
#[case(" but ", StepKeyword::But)]
fn parses_case_insensitively(#[case] input: &str, #[case] expected: StepKeyword) {
    assert!(matches!(StepKeyword::from_str(input), Ok(val) if val == expected));
}

#[rstest]
#[case(StepType::Given, StepKeyword::Given)]
#[case(StepType::When, StepKeyword::When)]
#[case(StepType::Then, StepKeyword::Then)]
fn maps_step_type(#[case] input: StepType, #[case] expected: StepKeyword) {
    assert_eq!(kw_from_type(input), expected);
}

#[test]
fn unsupported_step_type_display_mentions_variant() {
    let _guard = ScopedLocalization::new(&[langid!("en-US")])
        .unwrap_or_else(|error| panic!("failed to scope English locale: {error}"));
    let err = UnsupportedStepType(StepType::Then);
    let message = strip_directional_isolates(&err.to_string());
    assert!(
        message.contains("Then"),
        "display should include offending variant: {message}",
    );
    // Guard lives until function exit so the localisation context remains active.
}

#[test]
fn unsupported_step_type_is_an_error() {
    fn assert_error_trait<E: std::error::Error>(_: &E) {}
    let err = UnsupportedStepType(StepType::Then);
    assert_error_trait(&err);
    assert_eq!(err.0, StepType::Then);
}

#[test]
fn step_execution_from_value_without_payload() {
    match StepExecution::from_value(None) {
        StepExecution::Continue { value } => {
            assert!(value.is_none(), "expected empty payload");
        }
        StepExecution::Skipped { .. } => panic!("skip variant is unexpected"),
    }
}

#[test]
fn step_execution_from_value_with_payload() {
    let value = StepExecution::from_value(Some(Box::new(99_u8)));
    let payload = match value {
        StepExecution::Continue {
            value: Some(payload),
        } => payload,
        StepExecution::Continue { value: None } => {
            panic!("expected value to carry payload");
        }
        StepExecution::Skipped { .. } => panic!("skip variant is unexpected"),
    };
    #[expect(
        clippy::expect_used,
        reason = "test ensures payload can be downcast to original type"
    )]
    let number = payload.downcast::<u8>().expect("payload must be a u8");
    assert_eq!(*number, 99);
}

#[test]
fn step_execution_skipped_carries_message() {
    let message = String::from("not implemented");
    let outcome = StepExecution::skipped(Some(message.clone()));
    match outcome {
        StepExecution::Continue { .. } => panic!("continue variant is unexpected"),
        StepExecution::Skipped {
            message: Some(text),
        } => {
            assert_eq!(text, message);
        }
        StepExecution::Skipped { message: None } => {
            panic!("skip should carry the provided message");
        }
    }
}

#[test]
fn payload_from_value_returns_none_for_unit() {
    assert!(crate::__rstest_bdd_payload_from_value(()).is_none());
}

#[test]
fn payload_from_value_returns_none_for_unit_alias() {
    type UnitAlias = ();
    let value: UnitAlias = ();
    assert!(crate::__rstest_bdd_payload_from_value(value).is_none());
}

// Async type tests ---------------------------------------------------------

#[test]
fn step_future_can_be_constructed_from_ready_future() {
    // Verify that StepFuture can be created from an immediately-ready future.
    fn make_ready_future<'a>() -> StepFuture<'a> {
        Box::pin(std::future::ready(Ok(StepExecution::from_value(None))))
    }

    let future = make_ready_future();
    // Ensure the future type is correctly constructed (compile-time check).
    let _: StepFuture<'_> = future;
}

#[test]
fn async_step_fn_signature_is_valid() {
    // Verify that the AsyncStepFn type alias matches the expected signature.
    fn dummy_async_step<'a>(
        _ctx: &'a mut crate::context::StepContext<'a>,
        _text: &str,
        _docstring: Option<&str>,
        _table: Option<&[&[&str]]>,
    ) -> StepFuture<'a> {
        Box::pin(std::future::ready(Ok(StepExecution::from_value(None))))
    }

    // This assignment verifies the function signature matches AsyncStepFn.
    let _: AsyncStepFn = dummy_async_step;
}

#[test]
fn step_future_resolves_to_expected_value() {
    fn make_future<'a>() -> StepFuture<'a> {
        Box::pin(std::future::ready(Ok(StepExecution::from_value(Some(
            Box::new(42_u32),
        )))))
    }

    let future = make_future();
    // Poll the ready future to completion.
    let waker = std::task::Waker::noop();
    let mut cx = std::task::Context::from_waker(&waker);
    let mut pinned = future;
    match std::pin::Pin::as_mut(&mut pinned).poll(&mut cx) {
        std::task::Poll::Ready(Ok(StepExecution::Continue { value: Some(v) })) => {
            let num = v.downcast::<u32>().expect("expected u32");
            assert_eq!(*num, 42);
        }
        other => panic!("unexpected poll result: {other:?}"),
    }
}
