//! Tests for inferring step patterns from function names.

use rstest::rstest;
use rstest_bdd::{Step, StepKeyword, iter};
use rstest_bdd_macros::{given, then, when};

#[given]
fn user_logs_in() {}

#[when]
fn action_happens() {}

#[when("")]
fn explicit_empty_literal_is_respected() {}

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

#[rstest]
#[case(StepKeyword::Given, "user logs in")]
#[case(StepKeyword::When, "action happens")]
#[case(StepKeyword::Then, "result occurs")]
#[case(StepKeyword::Given, " leading underscore")]
#[case(StepKeyword::When, "trailing underscore ")]
#[case(StepKeyword::Then, "Consecutive  underscores")]
#[case(StepKeyword::Given, "with numbers 2")]
#[case(StepKeyword::Given, "match logs in")]
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
