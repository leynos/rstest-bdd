//! Runtime execution policy and helpers for scenario step execution.
//!
//! This module provides abstractions and utilities that were previously embedded
//! in the codegen layer. By moving them here, we achieve clearer separation of
//! concerns: the macro layer generates minimal glue code that delegates to these
//! runtime functions.
//!
//! # Key Components
//!
//! - [`RuntimeMode`]: Canonical definition of execution modes (sync, async
//!   variants), re-exported from `rstest_bdd_policy`.
//! - [`TestAttributeHint`]: Canonical definition for test attribute generation
//!   hints, re-exported from `rstest_bdd_policy`.
//! - [`StepExecutionRequest`]: Groups step data and diagnostic context for execution.
//! - Helper functions for step execution, fixture validation, and skip encoding.

mod error;

use std::any::Any;
use std::collections::HashSet;
use std::sync::Arc;

use crate::context::StepContext;
use crate::{
    Step, StepExecution, StepExecutionMode, StepKeyword, StepText, find_step_with_metadata,
};

pub use error::{ExecutionError, MissingFixturesDetails};

/// Prefix character for encoded skip messages with no message content.
pub(crate) const SKIP_NONE_PREFIX: char = '\u{0}';

/// Prefix character for encoded skip messages with message content.
pub(crate) const SKIP_SOME_PREFIX: char = '\u{1}';

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

/// Validate that all required fixtures are present in the context.
///
/// Returns an error if any required fixtures are missing from the context.
///
/// # Arguments
///
/// * `step` - The step definition containing fixture requirements
/// * `ctx` - The scenario context with available fixtures
/// * `request` - The step execution request for diagnostic context
///
/// # Errors
///
/// Returns [`ExecutionError::MissingFixtures`] if any fixture listed in
/// `step.fixtures` is not available in `ctx`.
fn validate_required_fixtures(
    step: &Step,
    ctx: &StepContext<'_>,
    request: &StepExecutionRequest<'_>,
) -> Result<(), ExecutionError> {
    if step.fixtures.is_empty() {
        return Ok(());
    }

    let available: HashSet<&str> = ctx.available_fixtures().collect();
    let missing: Vec<_> = step
        .fixtures
        .iter()
        .copied()
        .filter(|f| !available.contains(f))
        .collect();

    if missing.is_empty() {
        Ok(())
    } else {
        let mut available_list: Vec<_> = available.into_iter().map(String::from).collect();
        available_list.sort_unstable();
        Err(ExecutionError::MissingFixtures(Arc::new(
            MissingFixturesDetails {
                step_pattern: step.pattern.as_str().to_string(),
                step_location: format!("{}:{}", step.file, step.line),
                required: step.fixtures.to_vec(),
                missing,
                available: available_list,
                feature_path: request.feature_path.to_string(),
                scenario_name: request.scenario_name.to_string(),
            },
        )))
    }
}

fn resolve_step_for_request(
    request: &StepExecutionRequest<'_>,
) -> Result<&'static Step, ExecutionError> {
    find_step_with_metadata(request.keyword, StepText::from(request.text)).ok_or_else(|| {
        ExecutionError::StepNotFound {
            index: request.index,
            keyword: request.keyword,
            text: request.text.to_string(),
            feature_path: request.feature_path.to_string(),
            scenario_name: request.scenario_name.to_string(),
        }
    })
}

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
            text: request.text.to_string(),
            error: Arc::new(err),
            feature_path: request.feature_path.to_string(),
            scenario_name: request.scenario_name.to_string(),
        }),
    }
}

/// Encode a skip message for propagation through the executor loop.
///
/// The encoding uses prefix characters to distinguish between skip requests
/// with and without messages, allowing the skip signal to be transmitted
/// through `Result<_, String>` return types.
///
/// # Deprecation
///
/// This function is deprecated in favour of using [`ExecutionError::Skip`]
/// directly. The new error type provides structured skip handling without
/// string encoding.
///
/// # Arguments
///
/// * `message` - Optional skip message to encode
///
/// # Returns
///
/// An encoded string starting with [`SKIP_NONE_PREFIX`] (no message) or
/// [`SKIP_SOME_PREFIX`] followed by the message content.
///
/// # Examples
///
/// ```
/// use rstest_bdd::execution::{encode_skip_message, decode_skip_message};
///
/// // Round-trip with no message
/// let encoded = encode_skip_message(None);
/// assert_eq!(decode_skip_message(encoded), None);
///
/// // Round-trip with message
/// let encoded = encode_skip_message(Some("reason".to_string()));
/// assert_eq!(decode_skip_message(encoded), Some("reason".to_string()));
/// ```
#[deprecated(
    since = "0.8.0",
    note = "Use ExecutionError::Skip variant instead of string encoding"
)]
#[must_use]
pub fn encode_skip_message(message: Option<String>) -> String {
    message.map_or_else(
        || SKIP_NONE_PREFIX.to_string(),
        |msg| {
            let mut encoded = String::with_capacity(1 + msg.len());
            encoded.push(SKIP_SOME_PREFIX);
            encoded.push_str(&msg);
            encoded
        },
    )
}

/// Decode an encoded skip message.
///
/// Reverses the encoding performed by [`encode_skip_message`], extracting
/// the original message content from the prefixed format.
///
/// # Deprecation
///
/// This function is deprecated in favour of using [`ExecutionError::Skip`]
/// directly. The new error type provides structured skip handling without
/// string encoding. Use [`ExecutionError::skip_message`] to extract skip
/// messages from the new error type.
///
/// # Arguments
///
/// * `encoded` - The encoded skip message string
///
/// # Returns
///
/// `None` if the message was encoded without content, `Some(message)` otherwise.
/// Malformed input is returned as-is wrapped in `Some` for diagnostic purposes.
///
/// # Examples
///
/// ```
/// use rstest_bdd::execution::{encode_skip_message, decode_skip_message};
///
/// let encoded = encode_skip_message(Some("test".to_string()));
/// assert_eq!(decode_skip_message(encoded), Some("test".to_string()));
/// ```
#[deprecated(
    since = "0.8.0",
    note = "Use ExecutionError::skip_message() instead of string decoding"
)]
#[must_use]
pub fn decode_skip_message(encoded: String) -> Option<String> {
    match encoded.chars().next() {
        Some(c) if c == SKIP_NONE_PREFIX => None,
        Some(c) if c == SKIP_SOME_PREFIX => {
            // Safe: prefix_len is the byte length of the first char we just matched
            let prefix_len = c.len_utf8();
            Some(encoded.get(prefix_len..)?.to_string())
        }
        // Defensive fallback: preserve unexpected or malformed input rather than
        // panicking. This handles edge cases such as:
        // - Empty strings (no prefix character present)
        // - Future format changes where the prefix characters evolve
        // - Corrupted messages from unexpected runtime conditions
        // Returning the original input wrapped in `Some` allows downstream code
        // to inspect and diagnose the unexpected format.
        _ => Some(encoded),
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
/// - [`ExecutionError::Skip`]: The step requested skipping (control flow signal,
///   not an error). Use [`ExecutionError::is_skip`] to detect this case.
/// - [`ExecutionError::StepNotFound`]: No step matching the keyword and text
///   was found in the registry.
/// - [`ExecutionError::MissingFixtures`]: The step requires fixtures that are
///   not available in the context.
/// - [`ExecutionError::HandlerFailed`]: The step handler function returned an
///   error during execution.
///
/// # Examples
///
/// ```
/// use rstest_bdd::execution::{execute_step, ExecutionError, StepExecutionRequest};
/// use rstest_bdd::{StepContext, StepKeyword};
///
/// let request = StepExecutionRequest {
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

    handle_step_result(
        request,
        (step.run)(ctx, request.text, request.docstring, request.table),
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
pub async fn execute_step_async(
    request: &StepExecutionRequest<'_>,
    ctx: &mut StepContext<'_>,
) -> Result<Option<Box<dyn Any>>, ExecutionError> {
    let step = resolve_step_for_request(request)?;

    validate_required_fixtures(step, ctx, request)?;

    let result = match step.execution_mode {
        StepExecutionMode::Async => {
            (step.run_async)(ctx, request.text, request.docstring, request.table).await
        }
        StepExecutionMode::Sync | StepExecutionMode::Both => {
            (step.run)(ctx, request.text, request.docstring, request.table)
        }
    };

    handle_step_result(request, result)
}

#[cfg(test)]
mod tests;
