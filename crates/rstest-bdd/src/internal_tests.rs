//! Internal unit tests for crate-private helpers.
//! These tests validate the placeholder parser primitives and the
//! `IntoStepResult` specialisations that normalise step return values.
//! Grouping them here keeps the assertions close to the implementation
//! while preserving access to private items.

use crate::placeholder::{
    RegexBuilder, is_double_brace, is_escaped_brace, is_placeholder_start, parse_double_brace,
    parse_escaped_brace, parse_literal, parse_placeholder,
};
use crate::{IntoStepResult, NotResult};
use std::any::Any;
use std::fmt;

#[test]
fn predicates_detect_expected_tokens() {
    let s = br"\{\}{{}}{a}{_}";
    // Escaped braces
    assert!(is_escaped_brace(s, 0));
    assert!(!is_escaped_brace(s, 1));
    assert!(is_escaped_brace(s, 2));
    // Double braces
    assert!(is_double_brace(s, 4)); // "{{"
    assert!(is_double_brace(s, 6)); // "}}"
    // Placeholder start
    assert!(is_placeholder_start(s, 8)); // "{a"
    assert!(is_placeholder_start(s, 11)); // "{_"
}

#[test]
fn parse_escaped_and_double_braces() {
    // Escaped brace
    let mut st = RegexBuilder::new(r"\{");
    parse_escaped_brace(&mut st);
    assert_eq!(st.position, 2);
    assert!(st.output.ends_with(r"\{"));

    // Double brace
    let mut st2 = RegexBuilder::new("{{");
    parse_double_brace(&mut st2);
    assert_eq!(st2.position, 2);
    assert!(st2.output.ends_with(r"\{"));
}

#[test]
fn parse_placeholder_without_type_and_with_type() {
    // Without type; nested braces in placeholder content
    let mut st = RegexBuilder::new("before {outer {inner}} after");
    // Advance to the '{'
    st.position = "before ".len();
    #[expect(clippy::expect_used, reason = "test helper should fail loudly")]
    parse_placeholder(&mut st).expect("placeholder should parse");
    assert!(st.output.contains("(.+?)"));

    // With integer type
    let mut st2 = RegexBuilder::new("x {n:u32} y");
    st2.position = 2; // at '{'
    #[expect(clippy::expect_used, reason = "test helper should fail loudly")]
    parse_placeholder(&mut st2).expect("placeholder should parse");
    assert!(st2.output.contains(r"(\d+)"));
}

#[test]
fn parse_literal_writes_char() {
    let mut st = RegexBuilder::new("a");
    parse_literal(&mut st);
    assert_eq!(st.position, 1);
    assert!(st.output.ends_with('a'));
}

fn assert_not_result<T: NotResult>() {}

fn expect_ok_none(result: Result<Option<Box<dyn Any>>, String>) {
    match result {
        Ok(None) => (),
        Ok(Some(_)) => panic!("expected step result to be None"),
        Err(err) => panic!("expected Ok(None) but got error: {err}"),
    }
}

fn expect_ok_box(result: Result<Option<Box<dyn Any>>, String>) -> Box<dyn Any> {
    match result {
        Ok(Some(value)) => value,
        Ok(None) => panic!("expected step result to contain a value"),
        Err(err) => panic!("expected Ok(Some(_)) but got error: {err}"),
    }
}

fn expect_err(result: Result<Option<Box<dyn Any>>, String>) -> String {
    match result {
        Ok(Some(_)) => panic!("expected Err but got Ok(Some(_))"),
        Ok(None) => panic!("expected Err but got Ok(None)"),
        Err(err) => err,
    }
}

fn extract_value<T: 'static>(value: Box<dyn Any>) -> T {
    value
        .downcast::<T>()
        .map_or_else(|_| panic!("failed to downcast step value"), |v| *v)
}

// Macros keep the IntoStepResult assertions terse and consistent across the
// varied test cases.
macro_rules! assert_into_step_none {
    ($value:expr) => {{
        expect_ok_none(($value).into_step_result());
    }};
}

macro_rules! assert_into_step_value {
    ($value:expr => $ty:ty, $expected:expr) => {{
        let boxed = expect_ok_box(($value).into_step_result());
        let actual = extract_value::<$ty>(boxed);
        assert_eq!(actual, $expected);
    }};
}

macro_rules! assert_into_step_error {
    ($value:expr, $expected:expr) => {{
        let message = expect_err(($value).into_step_result());
        assert_eq!(message, $expected);
    }};
}

#[derive(Debug, PartialEq, Eq)]
struct CustomValue(u16);

#[derive(Debug, PartialEq, Eq)]
struct DisplayError(&'static str);

impl fmt::Display for DisplayError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.0)
    }
}

type AliasResult = Result<CustomValue, DisplayError>;

#[test]
fn unit_type_becomes_none() {
    assert_not_result::<()>();
    assert_into_step_none!(());
}

#[test]
fn option_type_uses_not_result_impl() {
    assert_not_result::<Option<i32>>();
    assert_into_step_value!(Some(5_i32) => Option<i32>, Some(5));
}

#[test]
fn custom_struct_round_trips() {
    assert_not_result::<CustomValue>();
    assert_into_step_value!(CustomValue(7) => CustomValue, CustomValue(7));
}

#[test]
fn result_ok_unit_maps_to_none() {
    let result: Result<(), &str> = Ok(());
    assert_into_step_none!(result);
}

#[test]
fn result_ok_value_boxes_payload() {
    let result: Result<i64, &str> = Ok(54);
    assert_into_step_value!(result => i64, 54);
}

#[test]
fn result_error_uses_display_message() {
    let result: Result<i32, DisplayError> = Err(DisplayError("boom"));
    assert_into_step_error!(result, "boom");
}

#[test]
fn type_alias_result_round_trips() {
    let ok: AliasResult = Ok(CustomValue(11));
    assert_into_step_value!(ok => CustomValue, CustomValue(11));

    let err: AliasResult = Err(DisplayError("alias failure"));
    assert_into_step_error!(err, "alias failure");
}

#[test]
fn primitive_value_round_trips() {
    assert_not_result::<i32>();
    let boxed = expect_ok_box(42_i32.into_step_result());
    let value = extract_value::<i32>(boxed);
    assert_eq!(value, 42);
}

#[test]
fn result_unit_string_error_maps() {
    let ok: Result<(), &str> = Ok(());
    expect_ok_none(ok.into_step_result());

    let err: Result<(), &str> = Err("boom");
    let message = expect_err(err.into_step_result());
    assert_eq!(message, "boom");
}

#[test]
fn result_string_specialisation_handles_payload_and_error() {
    let ok: Result<String, &str> = Ok("value".to_owned());
    let boxed = expect_ok_box(ok.into_step_result());
    let value = extract_value::<String>(boxed);
    assert_eq!(value, "value");

    let err: Result<String, &str> = Err("fail");
    let message = expect_err(err.into_step_result());
    assert_eq!(message, "fail");
}
