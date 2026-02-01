//! Behavioural tests for `ExecutionError` propagation through generated step loops.
//!
//! These tests verify that non-skip errors from `execute_step` cause the generated
//! step loop to panic with the error message, exercising the integration between
//! `ExecutionError` formatting and the generated `__rstest_bdd_extract_skip_message`
//! helper.
//!
//! # Error Types Tested
//!
//! - [`HandlerFailed`](rstest_bdd::execution::ExecutionError::HandlerFailed):
//!   Step handler returns an error (e.g., `Err("message")`)
//! - [`MissingFixtures`](rstest_bdd::execution::ExecutionError::MissingFixtures):
//!   Step requires fixtures not available in the scenario context
//!
//! # Note on `StepNotFound`
//!
//! The [`StepNotFound`](rstest_bdd::execution::ExecutionError::StepNotFound) error type
//! is validated at compile time by the `#[scenario]` macro, which emits a compile error
//! if a feature file references a step pattern not registered in the step registry.
//! Therefore, `StepNotFound` cannot occur at runtime through the generated scenario code.
//! This error type exists for direct runtime use of the step registry APIs.
//! See `tests/fixtures_macros/scenario_missing_step.rs` for compile-time validation tests.

use rstest_bdd_macros::{given, scenario, then, when};

// ============================================================================
// HandlerFailed error propagation
// ============================================================================

/// Step that always returns an error.
///
/// This triggers `ExecutionError::HandlerFailed` in the generated step loop,
/// which should panic with the formatted error message.
#[given("a step that will fail")]
fn step_that_fails() -> Result<(), &'static str> {
    Err("intentional failure for error propagation test")
}

/// Step that should never execute.
///
/// If this step runs, the test has failed to propagate the error correctly.
#[then("this step should not execute")]
fn should_not_execute() {
    panic!("error propagation failed - trailing step executed");
}

/// Verify that handler errors from step execution propagate as panics through
/// the generated step loop.
///
/// The test expects the panic message to contain the original error because:
/// 1. The step returns `Err("intentional failure...")`
/// 2. The generated wrapper converts this to `StepError::ExecutionError`
/// 3. `execute_step` wraps this in `ExecutionError::HandlerFailed`
/// 4. `__rstest_bdd_extract_skip_message` returns `None` (not a skip)
/// 5. The step loop panics with the error's `Display` output
#[scenario(path = "tests/features/step_execution_error.feature")]
#[should_panic(expected = "intentional failure for error propagation test")]
fn scenario_handler_error_propagates() {}

// ============================================================================
// MissingFixtures error propagation
// ============================================================================

/// Dummy step to satisfy the first step in the `missing_fixture` scenario.
#[given("a registered step")]
fn registered_step() {}

/// Step that requires a fixture parameter.
///
/// This step requires a fixture named `required_fixture` that is NOT provided by
/// the scenario, triggering `ExecutionError::MissingFixtures`.
#[when("a step needs fixture")]
fn step_needs_fixture(_required_fixture: &u32) {}

/// Verify that `MissingFixtures` errors propagate as panics through the generated step loop.
///
/// The test expects the panic message to contain "missing" because:
/// 1. The feature calls a step requiring fixture `required_fixture`
/// 2. No fixture with that name/type is available in the context
/// 3. `execute_step` returns `ExecutionError::MissingFixtures`
/// 4. `__rstest_bdd_extract_skip_message` returns `None` (not a skip)
/// 5. The step loop panics with the error's `Display` output
#[scenario(path = "tests/features/missing_fixture_error.feature")]
#[should_panic(expected = "missing")]
fn scenario_missing_fixtures_propagates() {}
