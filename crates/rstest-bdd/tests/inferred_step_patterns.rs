//! Behavioural tests for inferred step patterns.

use rstest::rstest;
use rstest_bdd::{StepContext, StepKeyword, find_step};
use rstest_bdd_macros::{given, then, when};

#[given]
fn user_starts_logged_out() {}

#[when]
fn user_logs_in() {}

#[when]
fn i_add_the_following_tasks() {}

#[then]
fn user_is_authenticated() {}

#[then(" ")]
fn whitespace_only_attribute_is_inferred() {}

#[given]
fn _leading_underscore() {}

#[when]
fn trailing_underscore_() {}

#[then]
#[expect(non_snake_case, reason = "test unusual function names")]
fn Consecutive__underscores() {}

#[given]
fn with_numbers_2() {}

#[when]
fn r#match_logs_in() {}

/// Executes registered steps using inferred patterns.
#[rstest]
#[case(StepKeyword::Given, "User starts logged out")]
#[case(StepKeyword::When, "User logs in")]
#[case(StepKeyword::Then, "User is authenticated")]
#[case(StepKeyword::Then, "Whitespace only attribute is inferred")]
#[case(StepKeyword::Given, " leading underscore")]
#[case(StepKeyword::When, "Trailing underscore ")]
#[case(StepKeyword::Then, "Consecutive  underscores")]
#[case(StepKeyword::Given, "With numbers 2")]
#[case(StepKeyword::When, "Match logs in")]
#[case(StepKeyword::When, "I add the following tasks")]
fn steps_with_inferred_patterns_execute(#[case] kw: StepKeyword, #[case] pattern: &str) {
    let ctx = StepContext::default();
    #[expect(clippy::expect_used, reason = "test ensures step exists")]
    let step_fn = find_step(kw, pattern.into()).expect("step not found");
    if let Err(e) = step_fn(&ctx, pattern, None, None) {
        panic!("step failed: {e:?}");
    }
}

/// Returns `None` when no step matches the pattern.
#[test]
fn find_step_returns_none_for_unknown_pattern() {
    assert!(find_step(StepKeyword::When, "user signs out".into()).is_none());
}
