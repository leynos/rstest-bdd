//! Runtime execution policy and helpers for scenario step execution.
//!
//! This module provides abstractions and utilities that were previously embedded
//! in the codegen layer. By moving them here, we achieve clearer separation of
//! concerns: the macro layer generates minimal glue code that delegates to these
//! runtime functions.
//!
//! # Key Components
//!
//! - [`RuntimeMode`]: Canonical definition of execution modes (sync, async variants), re-exported
//!   from `rstest_bdd_policy`.
//! - [`TestAttributeHint`]: Canonical definition for test attribute generation hints, re-exported
//!   from `rstest_bdd_policy`.
//! - [`StepExecutionRequest`]: Groups step data and diagnostic context for execution.
//! - Helper functions for step execution and fixture validation. `RuntimeMode` and
//!   `TestAttributeHint` are re-exported from `rstest_bdd_policy`, the shared source of truth used
//!   by the macro crate. Runtime and macro tests guard that single-source policy boundary.

mod error;
/// Fixture validation helpers for step execution.
mod fixtures;
mod unwind;

use std::{any::Any, sync::Arc};

pub use error::{ExecutionError, MissingFixtureDiagnostic, MissingFixturesDetails};
use fixtures::validate_required_fixtures;
/// Runtime mode for scenario test execution (canonical definition).
///
/// This type is re-exported from `rstest_bdd_policy` to keep the public API in
/// `rstest_bdd::execution` stable for downstream users.
pub use rstest_bdd_policy::RuntimeMode;
/// Hint for which test attributes the macro layer should generate
/// (canonical definition).
///
/// This type is re-exported from `rstest_bdd_policy` to keep the public API in
/// `rstest_bdd::execution` stable for downstream users.
pub use rstest_bdd_policy::TestAttributeHint;
use unwind::guarded;

use crate::{
    Step,
    StepExecution,
    StepExecutionMode,
    StepKeyword,
    StepScope,
    StepText,
    context::StepContext,
    find_step_with_metadata_in_scope,
    registry::mark_step_used,
};

/// Resolve the registered step described by an execution request.
fn resolve_step_for_request(
    request: &StepExecutionRequest<'_>,
) -> Result<&'static Step, ExecutionError> {
    let step = find_step_with_metadata_in_scope(
        request.scope,
        request.keyword,
        StepText::from(request.text),
    )
    .map_err(|error| ExecutionError::StepAmbiguous {
        message: error.to_string(),
    })?
    .ok_or_else(|| ExecutionError::StepNotFound {
        index: request.index,
        keyword: request.keyword,
        text: request.text.to_owned(),
        feature_path: request.feature_path.to_owned(),
        scenario_name: request.scenario_name.to_owned(),
    })?;
    mark_step_used(step);
    Ok(step)
}

/// Convert a step result into the executor's structured outcome.
fn handle_step_result(
    request: &StepExecutionRequest<'_>,
    result: Result<StepExecution, crate::StepError>,
) -> Result<Option<Box<dyn Any>>, ExecutionError> {
    match result {
        Ok(StepExecution::Skipped { message }) => Err(ExecutionError::Skip { message }),
        Ok(StepExecution::Continue { value }) => Ok(value),
        Err(err) => Err(ExecutionError::HandlerFailed {
            index: request.index,
            keyword: request.keyword,
            text: request.text.to_owned(),
            error: Arc::new(err),
            feature_path: request.feature_path.to_owned(),
            scenario_name: request.scenario_name.to_owned(),
        }),
    }
}

/// Groups step identification, data, and diagnostic context for execution.
///
/// This struct bundles all the information needed to execute a single scenario step,
/// reducing the parameter count of [`execute_step`] and making call sites more readable.
///
/// # Fields
///
/// * `index` - Zero-based step index for error messages
/// * `keyword` - The step keyword (Given, When, Then, etc.)
/// * `text` - The step text to match against patterns
/// * `docstring` - Optional docstring argument
/// * `table` - Optional data table argument
/// * `feature_path` - Path to the feature file for diagnostics
/// * `scenario_name` - Name of the scenario for diagnostics
#[derive(Debug)]
pub struct StepExecutionRequest<'a> {
    /// Zero-based step index for error messages.
    pub index: usize,
    /// The step keyword (Given, When, Then, etc.).
    pub keyword: StepKeyword,
    /// The step text to match against patterns.
    pub text: &'a str,
    /// Optional docstring argument.
    pub docstring: Option<&'a str>,
    /// Optional data table argument.
    pub table: Option<&'a [&'a [&'a str]]>,
    /// Path to the feature file for diagnostics.
    pub feature_path: &'a str,
    /// Name of the scenario for diagnostics.
    pub scenario_name: &'a str,
    /// Closed library scope selected by the generated scenario.
    pub scope: StepScope,
}

/// Execute a single step with validation and error handling.
///
/// This function encapsulates the core step execution logic:
/// 1. Look up the step in the registry
/// 2. Validate required fixtures are available
/// 3. Execute the step handler
/// 4. Handle the result (success, skip, or error)
///
/// # Arguments
///
/// * `request` - The step execution request containing all step data and context
/// * `ctx` - Mutable reference to the scenario context
///
/// # Returns
///
/// * `Ok(Some(value))` - Step succeeded and returned a value
/// * `Ok(None)` - Step succeeded without returning a value
/// * `Err(ExecutionError::Skip { .. })` - Step requested to be skipped
/// * `Err(ExecutionError::StepNotFound { .. })` - Step pattern not in registry
/// * `Err(ExecutionError::MissingFixtures(..))` - Required fixtures missing
/// * `Err(ExecutionError::HandlerFailed { .. })` - Step handler returned error
///
/// # Errors
///
/// Returns [`ExecutionError`] for all failure cases:
///
/// - [`ExecutionError::Skip`]: The step requested skipping (control flow signal, not an error). Use
///   [`ExecutionError::is_skip`] to detect this case.
/// - [`ExecutionError::StepNotFound`]: No step matching the keyword and text was found in the
///   registry.
/// - [`ExecutionError::MissingFixtures`]: The step requires fixtures that are not available in the
///   context.
/// - [`ExecutionError::HandlerFailed`]: The step handler function returned an error during
///   execution.
///
/// # Examples
///
/// ```
/// use rstest_bdd::{
///     StepContext,
///     StepKeyword,
///     StepScope,
///     execution::{ExecutionError, StepExecutionRequest, execute_step},
/// };
///
/// let request = StepExecutionRequest {
///     scope: StepScope::global(),
///     index: 0,
///     keyword: StepKeyword::Given,
///     text: "undefined step",
///     docstring: None,
///     table: None,
///     feature_path: "feature.feature",
///     scenario_name: "Scenario",
/// };
/// let mut ctx = StepContext::default();
///
/// let error = match execute_step(&request, &mut ctx) {
///     Err(error) => error,
///     Ok(_) => panic!("expected missing step"),
/// };
/// assert!(matches!(error, ExecutionError::StepNotFound { .. }));
/// ```
pub fn execute_step(
    request: &StepExecutionRequest<'_>,
    ctx: &mut StepContext<'_>,
) -> Result<Option<Box<dyn Any>>, ExecutionError> {
    let step = resolve_step_for_request(request)?;

    validate_required_fixtures(step, ctx, request)?;

    // The handler call is behind D11's boundary. A step registered through the
    // macro's own wrapper is caught there first and never reaches this one; a
    // step registered through the raw `step!` form has no wrapper, and without
    // this its panic would unwind straight out of `run_scenario`.
    handle_step_result(
        request,
        guarded(step, || {
            (step.run)(ctx, request.text, request.docstring, request.table)
        }),
    )
}

/// Execute a single step via the async handler with validation and error handling.
///
/// This mirrors [`execute_step`] but drives execution through the registered
/// async handler (`Step::run_async`) and awaits the returned future. This
/// enables true `async fn` step bodies under async scenario runtimes while
/// preserving the same fixture validation and error reporting behaviour.
///
/// # Errors
///
/// Returns [`ExecutionError`] for all failure cases, mirroring [`execute_step`].
///
/// # Examples
///
/// ```rust,no_run
/// use rstest_bdd::{
///     StepContext,
///     StepKeyword,
///     StepScope,
///     execution::{ExecutionError, StepExecutionRequest, execute_step_async},
/// };
///
/// let request = StepExecutionRequest {
///     scope: StepScope::global(),
///     index: 0,
///     keyword: StepKeyword::Given,
///     text: "undefined step",
///     docstring: None,
///     table: None,
///     feature_path: "feature.feature",
///     scenario_name: "Scenario",
/// };
/// let mut ctx = StepContext::default();
///
/// let runtime = tokio::runtime::Builder::new_current_thread()
///     .enable_all()
///     .build()
///     .expect("tokio runtime builds");
/// let error = runtime
///     .block_on(async { execute_step_async(&request, &mut ctx).await })
///     .unwrap_err();
/// assert!(matches!(error, ExecutionError::StepNotFound { .. }));
/// ```
pub async fn execute_step_async(
    request: &StepExecutionRequest<'_>,
    ctx: &mut StepContext<'_>,
) -> Result<Option<Box<dyn Any>>, ExecutionError> {
    let step = resolve_step_for_request(request)?;

    validate_required_fixtures(step, ctx, request)?;

    // Same boundary as `execute_step`, and for the same reason: an
    // `Async`-registered step's handler is a bare pointer too. It cannot use
    // `guarded`, though, because a genuine `async` body may await real I/O and
    // must keep yielding — so the unwind is caught per *poll* instead, which is
    // what `catch_unwind_future` exists for. The awaited-into `Ok` is then
    // mapped by the same `from_payload` the synchronous path uses, so the two
    // registration forms cannot classify a panic differently.
    let result = match step.execution_mode {
        StepExecutionMode::Async => {
            unwind::guarded_async(step, || {
                (step.run_async)(ctx, request.text, request.docstring, request.table)
            })
            .await
        }
        StepExecutionMode::Sync | StepExecutionMode::Both => guarded(step, || {
            (step.run)(ctx, request.text, request.docstring, request.table)
        }),
    };

    handle_step_result(request, result)
}

#[cfg(test)]
mod tests;
