//! Unit tests for shared core types and helper enums.

use super::*;
use crate::localization::{strip_directional_isolates, ScopedLocalization};
use gherkin::StepType;
use rstest::rstest;
use std::str::FromStr;
use unic_langid::langid;

fn parse_kw(input: &str) -> StepKeyword {
    StepKeyword::from_str(input)
        .unwrap_or_else(|e| panic!("failed to parse '{input}' as StepKeyword: {e}"))
}

fn kw_from_type(ty: StepType) -> StepKeyword {
    StepKeyword::try_from(ty)
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
    assert_eq!(parse_kw(input), expected);
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
    let guard = ScopedLocalization::new(&[langid!("en-US")])
        .unwrap_or_else(|error| panic!("failed to scope English locale: {error}"));
    let err = UnsupportedStepType(StepType::Then);
    let message = strip_directional_isolates(&err.to_string());
    assert!(
        message.contains("Then"),
        "display should include offending variant: {message}",
    );
    // Hold the localization context until the assertion completes.
    let _ = &guard;
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
