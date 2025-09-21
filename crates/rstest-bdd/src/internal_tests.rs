//! Internal unit tests for crate-private helpers.
//! These tests validate the `IntoStepResult` specialisations that normalise step return values.
//! Grouping them here keeps the assertions close to the implementation
//! while preserving access to private items.

use crate::{IntoStepResult, NotResult};
use std::any::Any;
use std::fmt;

mod into_step_result {
    //! Tests for `IntoStepResult` conversions covering fallback, unit, and result cases.
    use super::{expect_err, expect_ok_box, expect_ok_none, extract_value};
    use crate::IntoStepResult;

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    struct CustomType {
        x: i32,
        y: &'static str,
    }

    #[test]
    fn fallback_impl_boxes_custom_type() {
        let expected = CustomType { x: 7, y: "hello" };
        let boxed = expect_ok_box(expected.into_step_result());
        let value = extract_value::<CustomType>(boxed);
        assert_eq!(value, expected);
    }

    #[test]
    fn unit_specialisation_returns_none() {
        expect_ok_none(().into_step_result());
    }

    #[test]
    fn result_unit_specialisation_maps_errors() {
        expect_ok_none(Result::<(), &str>::Ok(()).into_step_result());
        let err = expect_err(Result::<(), &str>::Err("boom").into_step_result());
        assert_eq!(err, "boom");
    }

    #[test]
    fn result_non_unit_specialisation_propagates_errors() {
        let err = expect_err(Result::<CustomType, &str>::Err("custom fail").into_step_result());
        assert_eq!(err, "custom fail");
    }
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
