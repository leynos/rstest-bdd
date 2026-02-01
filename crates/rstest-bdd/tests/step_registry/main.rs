//! Behavioural test for step registry.

use rstest::rstest;
use rstest_bdd::localization::{ScopedLocalization, strip_directional_isolates};
use rstest_bdd::{
    Step, StepContext, StepError, StepExecution, StepKeyword, StepText, find_step_with_metadata,
    iter, unused_steps,
};
use unic_langid::langid;

#[path = "../common/mod.rs"]
mod common;
use common::poll_step_future;

mod execute_step_tests;
mod wrappers;

#[test]
fn step_is_registered() {
    let found = iter::<Step>
        .into_iter()
        .any(|step| step.pattern.as_str() == "behavioural" && step.keyword == StepKeyword::When);
    assert!(found, "expected step not found");
}

#[rstest]
#[case(StepKeyword::Given, "fails", "failing_wrapper", "boom", true)]
#[case(StepKeyword::When, "panics", "panicking_wrapper", "snap", false)]
#[case(
    StepKeyword::Then,
    "needs fixture",
    "needs_fixture",
    "Missing fixture 'missing' of type 'u32' for step function 'needs_fixture'",
    true
)]
fn wrapper_handles_panic_and_non_panic_errors(
    #[case] keyword: StepKeyword,
    #[case] pattern: &str,
    #[case] function_name: &str,
    #[case] expected_message: &str,
    #[case] expects_non_panic_branch: bool,
) {
    let step_fn = iter::<Step>
        .into_iter()
        .find(|s| s.pattern.as_str() == pattern && s.keyword == keyword)
        .map_or_else(
            || panic!("step '{pattern}' not found in registry"),
            |step| step.run,
        );
    let mut ctx = StepContext::default();
    let Err(err) = step_fn(&mut ctx, pattern, None, None) else {
        panic!("expected error from wrapper '{pattern}'");
    };
    let err_display = strip_directional_isolates(&err.to_string());
    if expects_non_panic_branch {
        match err {
            StepError::ExecutionError {
                pattern: p,
                function,
                message,
            } => {
                assert_eq!(p, pattern);
                assert_eq!(function, function_name);
                assert_eq!(message, expected_message);
            }
            StepError::MissingFixture { name, ty, step } => {
                assert_eq!(step, function_name);
                assert_eq!(name, "missing");
                assert_eq!(ty, "u32");
                assert_eq!(err_display, expected_message);
            }
            other => panic!("unexpected error for '{pattern}': {other:?}"),
        }
    } else {
        match err {
            StepError::PanicError {
                pattern: p,
                function,
                message,
            } => {
                assert_eq!(p, pattern);
                assert_eq!(function, function_name);
                assert_eq!(message, expected_message);
            }
            other => panic!("unexpected error for '{pattern}': {other:?}"),
        }
    }
}

#[rstest]
#[case(StepKeyword::Given, "fails", "Erreur lors de l'exécution de l'étape")]
#[case(StepKeyword::When, "panics", "Panique dans l'étape")]
#[case(
    StepKeyword::Then,
    "needs fixture",
    "La fixture « missing » de type « u32 » est introuvable"
)]
fn wrapper_errors_localize(
    #[case] keyword: StepKeyword,
    #[case] pattern: &str,
    #[case] expected_snippet: &str,
) {
    let _guard = ScopedLocalization::new(&[langid!("fr")])
        .unwrap_or_else(|error| panic!("failed to scope French locale: {error}"));
    let step_fn = iter::<Step>
        .into_iter()
        .find(|s| s.pattern.as_str() == pattern && s.keyword == keyword)
        .map_or_else(
            || panic!("step '{pattern}' not found in registry"),
            |step| step.run,
        );
    let mut ctx = StepContext::default();
    let Err(err) = step_fn(&mut ctx, pattern, None, None) else {
        panic!("expected error from wrapper '{pattern}'");
    };
    let message = strip_directional_isolates(&err.to_string());
    assert!(
        message.contains(expected_snippet),
        "expected '{expected_snippet}' in '{message}'",
    );
}

#[test]
#[expect(clippy::expect_used, reason = "step lookup must succeed for test")]
fn find_step_with_metadata_returns_step_with_fixtures() {
    let step = find_step_with_metadata(StepKeyword::Then, StepText::from("needs fixture"))
        .expect("step 'needs fixture' not found in registry");

    assert_eq!(step.pattern.as_str(), "needs fixture");
    assert_eq!(step.keyword, StepKeyword::Then);
    assert_eq!(step.fixtures, &["missing"]);
}

#[test]
fn find_step_with_metadata_returns_none_for_unknown_step() {
    let result = find_step_with_metadata(
        StepKeyword::Given,
        StepText::from("nonexistent step pattern"),
    );

    assert!(result.is_none());
}

#[test]
#[expect(clippy::expect_used, reason = "step lookup must succeed for test")]
fn find_step_with_metadata_returns_empty_fixtures_for_no_fixture_step() {
    let step = find_step_with_metadata(StepKeyword::When, StepText::from("behavioural"))
        .expect("step 'behavioural' not found in registry");

    assert_eq!(step.pattern.as_str(), "behavioural");
    assert!(step.fixtures.is_empty());
}

#[test]
fn available_fixtures_lists_scenario_fixtures() {
    let value_a = 42u32;
    let value_b = "hello";
    let mut ctx = StepContext::default();
    ctx.insert("fixture_a", &value_a);
    ctx.insert("fixture_b", &value_b);

    let available: std::collections::HashSet<_> = ctx.available_fixtures().collect();

    assert_eq!(available.len(), 2);
    assert!(available.contains("fixture_a"));
    assert!(available.contains("fixture_b"));
}

#[test]
#[expect(clippy::expect_used, reason = "step lookup must succeed for test")]
fn fixture_validation_detects_missing_fixtures() {
    // This test validates the fixture validation logic that is used in
    // execute_single_step() by replicating the same check here.
    let step = find_step_with_metadata(StepKeyword::Then, StepText::from("needs fixture"))
        .expect("step 'needs fixture' not found in registry");

    // Create a context with some fixtures but NOT the "missing" fixture
    let some_value = 123u32;
    let mut ctx = StepContext::default();
    ctx.insert("some_other_fixture", &some_value);

    let available: std::collections::HashSet<&str> = ctx.available_fixtures().collect();
    let missing: Vec<_> = step
        .fixtures
        .iter()
        .copied()
        .filter(|f| !available.contains(f))
        .collect();

    assert!(!missing.is_empty());
    assert!(missing.contains(&"missing"));
}

#[test]
#[expect(clippy::expect_used, reason = "step lookup must succeed for test")]
fn fixture_validation_passes_when_all_fixtures_present() {
    let step = find_step_with_metadata(StepKeyword::Then, StepText::from("needs fixture"))
        .expect("step 'needs fixture' not found in registry");

    // Create a context with the required "missing" fixture
    let value = 42u32;
    let mut ctx = StepContext::default();
    ctx.insert("missing", &value);

    let available: std::collections::HashSet<&str> = ctx.available_fixtures().collect();
    let missing: Vec<_> = step
        .fixtures
        .iter()
        .copied()
        .filter(|f| !available.contains(f))
        .collect();

    assert!(missing.is_empty());
}

#[test]
#[expect(clippy::expect_used, reason = "step lookup must succeed for test")]
fn find_step_with_metadata_marks_step_as_used() {
    // The step "needs fixture" should be marked as used after find_step_with_metadata
    let step = find_step_with_metadata(StepKeyword::Then, StepText::from("needs fixture"))
        .expect("step 'needs fixture' not found in registry");

    // Verify the step is no longer in the unused_steps list by comparing pointers.
    // Both `step` and items in `unused` are `&'static Step`, so we compare them directly.
    let unused = unused_steps();
    let is_still_unused = unused.iter().any(|s| std::ptr::eq(*s, step));

    assert!(
        !is_still_unused,
        "step 'needs fixture' should be marked as used after find_step_with_metadata"
    );
}

#[test]
fn step_with_auto_async_is_registered() {
    let found = iter::<Step>.into_iter().any(|step| {
        step.pattern.as_str() == "auto async step" && step.keyword == StepKeyword::Given
    });
    assert!(
        found,
        "expected step with auto-generated async wrapper not found"
    );
}

#[test]
#[expect(clippy::expect_used, reason = "test validates step lookup succeeds")]
fn step_with_auto_async_sync_handler_works() {
    let step = iter::<Step>
        .into_iter()
        .find(|s| s.pattern.as_str() == "auto async step" && s.keyword == StepKeyword::Given)
        .expect("step 'auto async step' not found in registry");

    let mut ctx = StepContext::default();
    let result = (step.run)(&mut ctx, "auto async step", None, None);
    assert!(result.is_ok(), "sync handler should succeed");
}

#[test]
#[expect(clippy::expect_used, reason = "test validates step lookup succeeds")]
fn step_with_auto_async_handler_works() {
    let step = iter::<Step>
        .into_iter()
        .find(|s| s.pattern.as_str() == "auto async step" && s.keyword == StepKeyword::Given)
        .expect("step 'auto async step' not found in registry");

    let mut ctx = StepContext::default();
    let future = (step.run_async)(&mut ctx, "auto async step", None, None);
    let result = poll_step_future(future);
    assert!(
        matches!(result, StepExecution::Continue { .. }),
        "auto-generated async handler failed: {result:?}"
    );
}
