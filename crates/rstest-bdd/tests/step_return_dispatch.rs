//! Structural regression tests for type-directed step return dispatch.

use std::{any::type_name_of_val, ops::Deref};

use rstest::rstest;
use rstest_bdd::{
    StepResult,
    step_return::{
        StepReturnNormalize,
        StepReturnProbe,
        StepReturnResultTag,
        StepReturnValueKind,
        StepReturnValueTag,
    },
};

type Alias<T> = Result<T, &'static str>;
type Inner<T> = Result<T, &'static str>;
type Outer = Inner<u8>;
type Defaulted<T = u8> = Result<T, &'static str>;
type Score = u32;

struct Newtype(Result<u8, &'static str>);

struct DerefResult(Result<u8, &'static str>);

impl Deref for DerefResult {
    type Target = Result<u8, &'static str>;

    fn deref(&self) -> &Self::Target { &self.0 }
}

macro_rules! selected_tag_name {
    ($value:expr) => {{
        let value = $value;
        type_name_of_val(&StepReturnProbe(&value).__rstest_bdd_step_return_kind())
    }};
}

// Guards invariant 1 of `step_return.rs`: aliases select the inherent Result arm.
fn unit_tag() -> &'static str { selected_tag_name!(()) }

// Guards invariant 1 of `step_return.rs`: ordinary values select the fallback arm.
fn value_tag() -> &'static str { selected_tag_name!(7_u8) }

// Guards invariant 1 of `step_return.rs`: value aliases remain values.
fn value_alias_tag() -> &'static str { selected_tag_name!(7 as Score) }

// Guards invariant 1 of `step_return.rs`: spelled Results select the inherent arm.
fn result_tag() -> &'static str { selected_tag_name!(Ok::<u8, &'static str>(7)) }

// Guards invariant 1 of `step_return.rs`: StepResult aliases select the Result arm.
fn step_result_tag() -> &'static str {
    let value: StepResult<u8, &'static str> = Ok(7);
    selected_tag_name!(value)
}

// Guards invariant 1 of `step_return.rs`: local Result aliases select the Result arm.
fn alias_tag() -> &'static str {
    let value: Alias<u8> = Ok(7);
    selected_tag_name!(value)
}

// Guards invariant 1 of `step_return.rs`: nested aliases select the Result arm.
fn nested_alias_tag() -> &'static str {
    let value: Outer = Ok(7);
    selected_tag_name!(value)
}

// Guards invariant 1 of `step_return.rs`: aliases with defaults select the Result arm.
fn defaulted_alias_tag() -> &'static str {
    let value: Defaulted = Ok(7);
    selected_tag_name!(value)
}

// Guards invariant 1 of `step_return.rs`: qualified Result aliases select the Result arm.
fn io_result_tag() -> &'static str {
    let value: std::io::Result<()> = Ok(());
    selected_tag_name!(value)
}

// Guards invariant 1 of `step_return.rs`: references are not dereferenced for dispatch.
fn reference_result_tag() -> &'static str {
    let result = Ok::<u8, &'static str>(7);
    selected_tag_name!(&result)
}

// Guards invariant 1 of `step_return.rs`: boxed Results remain values.
fn boxed_result_tag() -> &'static str { selected_tag_name!(Box::new(Ok::<u8, &'static str>(7))) }

// Guards invariant 1 of `step_return.rs`: optional Results remain values.
fn optional_result_tag() -> &'static str { selected_tag_name!(Some(Ok::<u8, &'static str>(7))) }

// Guards invariant 1 of `step_return.rs`: newtypes around Results remain values.
fn newtype_tag() -> &'static str {
    let value = Newtype(Ok(7));
    let _ = &value.0;
    selected_tag_name!(value)
}

// Guards invariant 1 of `step_return.rs`: deref targets are not followed.
fn deref_result_tag() -> &'static str { selected_tag_name!(DerefResult(Ok(7))) }

#[rstest]
#[case::unit(unit_tag, type_name_of_val(&StepReturnValueTag))]
#[case::plain_value(value_tag, type_name_of_val(&StepReturnValueTag))]
#[case::value_alias(value_alias_tag, type_name_of_val(&StepReturnValueTag))]
#[case::spelled_result(result_tag, type_name_of_val(&StepReturnResultTag))]
#[case::step_result(step_result_tag, type_name_of_val(&StepReturnResultTag))]
#[case::local_alias(alias_tag, type_name_of_val(&StepReturnResultTag))]
#[case::nested_alias(nested_alias_tag, type_name_of_val(&StepReturnResultTag))]
#[case::defaulted_alias(defaulted_alias_tag, type_name_of_val(&StepReturnResultTag))]
#[case::io_result(io_result_tag, type_name_of_val(&StepReturnResultTag))]
#[case::reference_result(reference_result_tag, type_name_of_val(&StepReturnValueTag))]
#[case::boxed_result(boxed_result_tag, type_name_of_val(&StepReturnValueTag))]
#[case::optional_result(optional_result_tag, type_name_of_val(&StepReturnValueTag))]
#[case::newtype(newtype_tag, type_name_of_val(&StepReturnValueTag))]
#[case::deref_result(deref_result_tag, type_name_of_val(&StepReturnValueTag))]
fn selects_the_expected_tag(
    #[case] selected_tag: fn() -> &'static str,
    #[case] expected_tag: &'static str,
) {
    assert_eq!(selected_tag(), expected_tag);
}

#[test]
fn result_normalization_preserves_display_message() {
    struct Error;

    impl std::fmt::Display for Error {
        fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            formatter.write_str("display message")
        }
    }

    impl std::fmt::Debug for Error {
        fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            formatter.write_str("debug message")
        }
    }

    let normalized = StepReturnNormalize::normalize(StepReturnResultTag, Err::<(), _>(Error));
    match normalized {
        Err(message) => assert_eq!(message, "display message"),
        Ok(_) => panic!("an Err return must remain a scenario failure"),
    }
}
