//! Tests for inferring step patterns from function names.

use rstest_bdd::{Step, StepKeyword, iter};
use rstest_bdd_macros::{given, then, when};

#[given]
fn user_logs_in() {}

#[when]
fn action_happens() {}

#[then]
fn result_occurs() {}

#[given]
fn _leading_underscore() {}

#[when]
fn trailing_underscore_() {}

#[then]
#[expect(non_snake_case, reason = "test unusual function names")]
fn consecutive__underscores() {}

#[given]
fn with_numbers_2() {}

#[test]
fn macros_register_inferred_steps() {
    let cases = [
        (StepKeyword::Given, "user logs in"),
        (StepKeyword::When, "action happens"),
        (StepKeyword::Then, "result occurs"),
        (StepKeyword::Given, " leading underscore"),
        (StepKeyword::When, "trailing underscore "),
        (StepKeyword::Then, "consecutive  underscores"),
        (StepKeyword::Given, "with numbers 2"),
    ];

    for (keyword, pattern) in cases {
        assert!(
            iter::<Step>
                .into_iter()
                .any(|s| s.keyword == keyword && s.pattern.as_str() == pattern),
            "Step not registered: {} {}",
            keyword.as_str(),
            pattern
        );
    }
}
