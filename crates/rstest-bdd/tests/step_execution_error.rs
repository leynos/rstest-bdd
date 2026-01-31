//! Behavioural tests for `ExecutionError` propagation through generated step loops.
//!
//! These tests verify that non-skip errors from `execute_step` cause the generated
//! step loop to panic with the error message, exercising the integration between
//! `ExecutionError` formatting and the generated `__rstest_bdd_extract_skip_message`
//! helper.

use rstest_bdd_macros::{given, scenario, then};

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
