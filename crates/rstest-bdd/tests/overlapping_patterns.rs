//! Integration tests for specificity-based pattern matching.
//!
//! These tests validate that when multiple patterns match the same step text,
//! the most specific pattern (more literal text, fewer placeholders) is selected.

use std::sync::atomic::{AtomicUsize, Ordering};

use rstest_bdd::{StepContext, StepExecution, StepKeyword, find_step};
use rstest_bdd_macros::given;
use serial_test::serial;

// Counters to track which step was executed
static WORKSPACE_EXECUTABLE_CALLED: AtomicUsize = AtomicUsize::new(0);
static GENERIC_OUTPUT_CALLED: AtomicUsize = AtomicUsize::new(0);
static VERY_GENERIC_CALLED: AtomicUsize = AtomicUsize::new(0);

static TYPED_PLACEHOLDER_CALLED: AtomicUsize = AtomicUsize::new(0);
static UNTYPED_PLACEHOLDER_CALLED: AtomicUsize = AtomicUsize::new(0);

// Pattern from issue #350: workspace executable pattern should beat generic
#[given("the stdlib output is the workspace executable {path}")]
fn workspace_executable_step(path: String) {
    drop(path);
    WORKSPACE_EXECUTABLE_CALLED.fetch_add(1, Ordering::Relaxed);
}

#[given("the stdlib output is {expected}")]
fn generic_output_step(expected: String) {
    drop(expected);
    GENERIC_OUTPUT_CALLED.fetch_add(1, Ordering::Relaxed);
}

#[given("{output}")]
fn very_generic_step(output: String) {
    drop(output);
    VERY_GENERIC_CALLED.fetch_add(1, Ordering::Relaxed);
}

// Test typed vs untyped placeholders
#[given("I have {count:u32} items")]
fn typed_placeholder_step(count: u32) {
    let _ = count;
    TYPED_PLACEHOLDER_CALLED.fetch_add(1, Ordering::Relaxed);
}

#[given("I have {count} items")]
fn untyped_placeholder_step(count: String) {
    drop(count);
    UNTYPED_PLACEHOLDER_CALLED.fetch_add(1, Ordering::Relaxed);
}

fn reset_counters() {
    WORKSPACE_EXECUTABLE_CALLED.store(0, Ordering::Relaxed);
    GENERIC_OUTPUT_CALLED.store(0, Ordering::Relaxed);
    VERY_GENERIC_CALLED.store(0, Ordering::Relaxed);
    TYPED_PLACEHOLDER_CALLED.store(0, Ordering::Relaxed);
    UNTYPED_PLACEHOLDER_CALLED.store(0, Ordering::Relaxed);
}

#[expect(clippy::expect_used, reason = "test helper ensures step exists")]
fn assert_step_execution(
    step_text: &str,
    expected_counters: (usize, usize, usize),
    assertion_message: &str,
) {
    let step_fn = find_step(StepKeyword::Given, step_text.into()).expect("step not found");

    let mut ctx = StepContext::default();
    match step_fn(&mut ctx, step_text, None, None) {
        Ok(StepExecution::Continue { .. }) => {}
        Ok(StepExecution::Skipped { .. }) => panic!("step unexpectedly skipped"),
        Err(e) => panic!("unexpected error: {e:?}"),
    }

    let workspace = WORKSPACE_EXECUTABLE_CALLED.load(Ordering::Relaxed);
    let generic = GENERIC_OUTPUT_CALLED.load(Ordering::Relaxed);
    let very_generic = VERY_GENERIC_CALLED.load(Ordering::Relaxed);

    assert_eq!(
        (workspace, generic, very_generic),
        expected_counters,
        "{assertion_message}"
    );
}

#[expect(clippy::expect_used, reason = "test helper ensures step exists")]
fn assert_typed_step_execution(
    step_text: &str,
    expected_counters: (usize, usize),
    assertion_message: &str,
) {
    let step_fn = find_step(StepKeyword::Given, step_text.into()).expect("step not found");

    let mut ctx = StepContext::default();
    match step_fn(&mut ctx, step_text, None, None) {
        Ok(StepExecution::Continue { .. }) => {}
        Ok(StepExecution::Skipped { .. }) => panic!("step unexpectedly skipped"),
        Err(e) => panic!("unexpected error: {e:?}"),
    }

    let typed = TYPED_PLACEHOLDER_CALLED.load(Ordering::Relaxed);
    let untyped = UNTYPED_PLACEHOLDER_CALLED.load(Ordering::Relaxed);

    assert_eq!((typed, untyped), expected_counters, "{assertion_message}");
}

#[test]
#[serial]
fn specific_pattern_beats_generic_from_issue_350() {
    reset_counters();
    assert_step_execution(
        "the stdlib output is the workspace executable /usr/bin/foo",
        (1, 0, 0),
        "workspace executable pattern (45 literal chars) must win over generic (21 literal chars)",
    );
}

#[test]
#[serial]
fn generic_pattern_matches_when_specific_does_not() {
    reset_counters();
    assert_step_execution(
        "the stdlib output is something else",
        (0, 1, 0),
        "generic pattern should match when specific pattern does not",
    );
}

#[test]
#[serial]
fn typed_placeholder_beats_untyped_as_tiebreaker() {
    reset_counters();
    assert_typed_step_execution(
        "I have 42 items",
        (1, 0),
        "typed placeholder should win as tiebreaker when literal counts are equal",
    );
}

#[test]
#[serial]
fn most_specific_wins_among_three_patterns() {
    reset_counters();
    // This text matches all three patterns:
    // - "the stdlib output is the workspace executable {path}" (45 literals)
    // - "the stdlib output is {expected}" (21 literals)
    // - "{output}" (0 literals)
    assert_step_execution(
        "the stdlib output is the workspace executable test/path",
        (1, 0, 0),
        "most specific pattern must win among all three candidates",
    );
}
