//! Tests for inferring step patterns from function names.

use rstest_bdd::{Step, StepKeyword, iter};
use rstest_bdd_macros::{given, then, when};

#[given]
fn user_logs_in() {}

#[when]
fn action_happens() {}

#[then]
fn result_occurs() {}

#[test]
fn macros_register_inferred_steps() {
    let cases = [
        (StepKeyword::Given, "user logs in"),
        (StepKeyword::When, "action happens"),
        (StepKeyword::Then, "result occurs"),
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
