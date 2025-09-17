//! Tests for inferring step patterns from function names.

use rstest::rstest;
use rstest_bdd::{Step, StepKeyword, iter};
use rstest_bdd_macros::{given, then, when};

#[given]
fn user_logs_in() {}

#[when]
fn action_happens() {}

#[when]
fn i_add_the_following_tasks() {}

#[when("")]
fn explicit_empty_literal_is_respected() {}

#[then(" ")]
fn whitespace_only_attribute_is_inferred() {}

#[then]
fn result_occurs() {}

#[given]
fn _leading_underscore() {}

#[when]
fn trailing_underscore_() {}

#[then]
#[expect(non_snake_case, reason = "test unusual function names")]
fn Consecutive__underscores() {}

#[given]
fn with_numbers_2() {}

#[given]
fn r#match_logs_in() {}

#[given]
fn überraschung() {}

#[rstest]
#[case(StepKeyword::Given, "User logs in")]
#[case(StepKeyword::When, "Action happens")]
#[case(StepKeyword::Then, "Result occurs")]
#[case(StepKeyword::Then, "Whitespace only attribute is inferred")]
#[case(StepKeyword::Given, " leading underscore")]
#[case(StepKeyword::When, "Trailing underscore ")]
#[case(StepKeyword::Then, "Consecutive  underscores")]
#[case(StepKeyword::Given, "With numbers 2")]
#[case(StepKeyword::Given, "Match logs in")]
#[case(StepKeyword::Given, "überraschung")]
#[case(StepKeyword::When, "I add the following tasks")]
#[case(StepKeyword::When, "")]
fn macros_register_inferred_steps(#[case] keyword: StepKeyword, #[case] pattern: &str) {
    assert!(
        iter::<Step>
            .into_iter()
            .any(|s| s.keyword == keyword && s.pattern.as_str() == pattern),
        "Step not registered: {} {}",
        keyword.as_str(),
        pattern
    );
}

#[test]
fn inferred_macro_step_records_original_function_name() {
    let Some(step) = iter::<Step>
        .into_iter()
        .find(|s| {
            s.keyword == StepKeyword::When && s.pattern.as_str() == "I add the following tasks"
        })
    else {
        panic!("expected step for inferred pattern");
    };
    assert_eq!(step.name, "i_add_the_following_tasks");
}
