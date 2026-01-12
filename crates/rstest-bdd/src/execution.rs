//! Runtime execution policy and helpers for scenario step execution.
//!
//! This module provides abstractions and utilities that were previously embedded
//! in the codegen layer. By moving them here, we achieve clearer separation of
//! concerns: the macro layer generates minimal glue code that delegates to these
//! runtime functions.
//!
//! # Key Components
//!
//! - [`RuntimeMode`]: Canonical definition of execution modes (sync, async variants).
//!   This is the runtime authoritative source; the macro crate maintains a parallel
//!   enum for compile-time use (see `rstest_bdd_macros::macros::scenarios::macro_args`).
//! - [`TestAttributeHint`]: Canonical definition for test attribute generation hints.
//!   Also mirrored in the macro crate for compile-time attribute decisions.
//! - [`StepExecutionRequest`]: Groups step data and diagnostic context for execution.
//! - Helper functions for step execution, fixture validation, and skip encoding.
//!
//! # Enum Synchronisation
//!
//! The [`RuntimeMode`] and [`TestAttributeHint`] enums are intentionally duplicated
//! in the macro crate (`rstest_bdd_macros::macros::scenarios::macro_args`). This
//! duplication is necessary because proc-macro crates cannot depend on runtime crates
//! at compile time. **Both definitions must be kept in sync manually.** When adding
//! new variants or changing semantics, update both locations and their tests.

use std::any::Any;
use std::collections::HashSet;

use crate::context::StepContext;
use crate::{Step, StepExecution, StepKeyword, StepText, find_step_with_metadata};

/// Prefix character for encoded skip messages with no message content.
const SKIP_NONE_PREFIX: char = '\u{0}';

/// Prefix character for encoded skip messages with message content.
const SKIP_SOME_PREFIX: char = '\u{1}';

/// Runtime mode for scenario test execution (canonical definition).
///
/// This enum represents the available execution strategies for scenarios.
/// It serves as the runtime authoritative definition for execution policy.
///
/// **Note:** The macro crate (`rstest_bdd_macros`) maintains a parallel enum
/// for compile-time use. Both must be kept in sync—see the module-level
/// documentation for details.
///
/// # Examples
///
/// ```
/// use rstest_bdd::execution::RuntimeMode;
///
/// let mode = RuntimeMode::default();
/// assert!(!mode.is_async());
///
/// let async_mode = RuntimeMode::TokioCurrentThread;
/// assert!(async_mode.is_async());
/// ```
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum RuntimeMode {
    /// Synchronous execution (default).
    #[default]
    Sync,
    /// Tokio current-thread runtime (`#[tokio::test(flavor = "current_thread")]`).
    TokioCurrentThread,
}

impl RuntimeMode {
    /// Returns `true` if this mode requires async test generation.
    ///
    /// # Examples
    ///
    /// ```
    /// use rstest_bdd::execution::RuntimeMode;
    ///
    /// assert!(!RuntimeMode::Sync.is_async());
    /// assert!(RuntimeMode::TokioCurrentThread.is_async());
    /// ```
    #[must_use]
    pub const fn is_async(self) -> bool {
        matches!(self, Self::TokioCurrentThread)
    }

    /// Returns a hint for which test attributes to generate.
    ///
    /// This is used by the macro layer to emit appropriate test attributes
    /// at compile time while keeping the policy decision centralised here.
    ///
    /// # Examples
    ///
    /// ```
    /// use rstest_bdd::execution::{RuntimeMode, TestAttributeHint};
    ///
    /// assert_eq!(
    ///     RuntimeMode::Sync.test_attribute_hint(),
    ///     TestAttributeHint::RstestOnly
    /// );
    /// assert_eq!(
    ///     RuntimeMode::TokioCurrentThread.test_attribute_hint(),
    ///     TestAttributeHint::RstestWithTokioCurrentThread
    /// );
    /// ```
    #[must_use]
    pub const fn test_attribute_hint(self) -> TestAttributeHint {
        match self {
            Self::Sync => TestAttributeHint::RstestOnly,
            Self::TokioCurrentThread => TestAttributeHint::RstestWithTokioCurrentThread,
        }
    }
}

/// Hint for which test attributes the macro layer should generate (canonical definition).
///
/// This enum provides a compile-time bridge between runtime policy decisions
/// and macro-generated test attributes.
///
/// **Note:** The macro crate (`rstest_bdd_macros`) maintains a parallel enum
/// for compile-time use. Both must be kept in sync—see the module-level
/// documentation for details.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TestAttributeHint {
    /// Generate only `#[rstest::rstest]`.
    RstestOnly,
    /// Generate `#[rstest::rstest]` and `#[tokio::test(flavor = "current_thread")]`.
    RstestWithTokioCurrentThread,
}

/// Validate that all required fixtures are present in the context.
///
/// Panics with a detailed diagnostic message if any required fixtures are missing.
///
/// # Arguments
///
/// * `step` - The step definition containing fixture requirements
/// * `ctx` - The scenario context with available fixtures
/// * `request` - The step execution request for diagnostic context
///
/// # Panics
///
/// Panics if any fixture listed in `step.fixtures` is not available in `ctx`.
fn validate_required_fixtures(
    step: &Step,
    ctx: &StepContext<'_>,
    request: &StepExecutionRequest<'_>,
) {
    if step.fixtures.is_empty() {
        return;
    }

    let available: HashSet<&str> = ctx.available_fixtures().collect();
    let missing: Vec<_> = step
        .fixtures
        .iter()
        .copied()
        .filter(|f| !available.contains(f))
        .collect();

    if !missing.is_empty() {
        let mut available_list: Vec<_> = available.into_iter().collect();
        available_list.sort_unstable();
        panic!(
            concat!(
                "Step '{}' (defined at {}:{}) requires fixtures {:?}, ",
                "but the following are missing: {:?}\n",
                "Available fixtures from scenario: {:?}\n",
                "(feature: {}, scenario: {})",
            ),
            request.text,
            step.file,
            step.line,
            step.fixtures,
            missing,
            available_list,
            request.feature_path,
            request.scenario_name,
        );
    }
}

/// Encode a skip message for propagation through the executor loop.
///
/// The encoding uses prefix characters to distinguish between skip requests
/// with and without messages, allowing the skip signal to be transmitted
/// through `Result<_, String>` return types.
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
/// * `Err(encoded_skip)` - Step requested a skip (use [`decode_skip_message`])
///
/// # Errors
///
/// Returns `Err` containing an encoded skip message when the step requests
/// skipping via [`StepExecution::Skipped`]. This is not an error condition
/// but a control flow signal. Use [`decode_skip_message`] to extract the
/// optional skip reason.
///
/// # Panics
///
/// Panics if:
/// - The step is not found in the registry
/// - Required fixtures are missing
/// - The step handler returns an error
pub fn execute_step(
    request: &StepExecutionRequest<'_>,
    ctx: &mut StepContext<'_>,
) -> Result<Option<Box<dyn Any>>, String> {
    let step = find_step_with_metadata(request.keyword, StepText::from(request.text))
        .unwrap_or_else(|| {
            panic!(
                "Step not found at index {}: {} {} (feature: {}, scenario: {})",
                request.index,
                request.keyword.as_str(),
                request.text,
                request.feature_path,
                request.scenario_name
            )
        });

    validate_required_fixtures(step, ctx, request);

    match (step.run)(ctx, request.text, request.docstring, request.table) {
        Ok(StepExecution::Skipped { message }) => Err(encode_skip_message(message)),
        Ok(StepExecution::Continue { value }) => Ok(value),
        Err(err) => {
            panic!(
                "Step failed at index {}: {} {} - {}\n(feature: {}, scenario: {})",
                request.index,
                request.keyword.as_str(),
                request.text,
                err,
                request.feature_path,
                request.scenario_name
            );
        }
    }
}

#[cfg(test)]
mod tests;
