//! Behavioural tests for inferred step patterns.

use rstest_bdd::{StepContext, StepKeyword, find_step};
use rstest_bdd_macros::{given, then, when};

#[given]
fn user_starts_logged_out() {}

#[when]
fn user_logs_in() {}

#[then]
fn user_is_authenticated() {}

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

#[test]
fn steps_with_inferred_patterns_execute() {
    let ctx = StepContext::default();
    for (kw, pattern) in [
        (StepKeyword::Given, "user starts logged out"),
        (StepKeyword::When, "user logs in"),
        (StepKeyword::Then, "user is authenticated"),
        (StepKeyword::Given, " leading underscore"),
        (StepKeyword::When, "trailing underscore "),
        (StepKeyword::Then, "Consecutive  underscores"),
        (StepKeyword::Given, "with numbers 2"),
        (StepKeyword::When, "match logs in"),
    ] {
        #[expect(clippy::expect_used, reason = "test ensures step exists")]
        let step_fn = find_step(kw, pattern.into()).expect("step not found");
        if let Err(e) = step_fn(&ctx, pattern, None, None) {
            panic!("step failed: {e:?}");
        }
    }
}
