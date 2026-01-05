//! Step execution code generators.
//!
//! This module generates the runtime Rust code responsible for executing individual
//! scenario steps. It receives step metadata (keywords, text patterns, fixtures) from
//! the scenario parser and produces `TokenStream2` fragments that are injected into
//! the generated test function body.
//!
//! # Responsibilities
//!
//! - Generate `__rstest_bdd_execute_single_step`: the main step executor that performs
//!   registry lookup via `find_step_with_metadata`, validates fixture availability,
//!   runs the step, and encodes skip signals for propagation.
//! - Generate `__rstest_bdd_decode_skip_message`: decodes encoded skip messages back
//!   into their original `Option<String>` form.
//! - Generate the step executor loop that iterates over scenario steps, dispatching
//!   each to the executor and handling results (value insertion or skip propagation).
//!
//! # Integration
//!
//! These generators are called by the parent `runtime` module during scenario codegen.
//! The produced tokens are combined with scenario-level scaffolding (from `scenario.rs`)
//! to form the complete test function body. Inner helper functions like
//! `validate_required_fixtures` and `encode_skip_message` are defined within the
//! generated executor to keep the public API minimal.
//!
//! # Invariants
//!
//! - Skip messages are encoded with `__RSTEST_BDD_SKIP_NONE_PREFIX` (no message) or
//!   `__RSTEST_BDD_SKIP_SOME_PREFIX` (message present) to distinguish skip signals
//!   from execution errors in the `Result<_, String>` return type.
//! - Fixture validation occurs before step execution, ensuring missing fixtures
//!   produce clear panic messages with diagnostic context.

use proc_macro2::TokenStream as TokenStream2;
use quote::quote;

/// Generates the `validate_required_fixtures` inner function.
///
/// The generated function checks that all fixtures required by a step are
/// available in the scenario context, panicking with a detailed message if
/// any are missing.
///
/// # Generated code
///
/// ```text
/// fn validate_required_fixtures(step, ctx, text, feature_path, scenario_name) {
///     // Early return if no fixtures required
///     // Collect available fixtures from context
///     // Find missing fixtures
///     // Panic with details if any missing
/// }
/// ```
fn generate_validate_fixtures_fn() -> TokenStream2 {
    let path = crate::codegen::rstest_bdd_path();
    quote! {
        fn validate_required_fixtures(
            step: &#path::Step,
            ctx: &#path::StepContext,
            text: &str,
            feature_path: &str,
            scenario_name: &str,
        ) {
            if step.fixtures.is_empty() {
                return;
            }

            let available: std::collections::HashSet<&str> =
                ctx.available_fixtures().collect();
            let missing: Vec<_> = step.fixtures
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
                    text,
                    step.file,
                    step.line,
                    step.fixtures,
                    missing,
                    available_list,
                    feature_path,
                    scenario_name,
                );
            }
        }
    }
}

/// Generates the `encode_skip_message` inner function.
///
/// The generated function encodes an optional skip message into a string
/// with a prefix character indicating whether a message is present.
///
/// # Generated code
///
/// ```text
/// fn encode_skip_message(message: Option<String>) -> String {
///     // Returns __RSTEST_BDD_SKIP_NONE_PREFIX if None
///     // Returns __RSTEST_BDD_SKIP_SOME_PREFIX + message if Some
/// }
/// ```
fn generate_encode_skip_fn() -> TokenStream2 {
    quote! {
        fn encode_skip_message(message: Option<String>) -> String {
            message.map_or_else(
                || __RSTEST_BDD_SKIP_NONE_PREFIX.to_string(),
                |msg| {
                    let mut encoded = String::with_capacity(1 + msg.len());
                    encoded.push(__RSTEST_BDD_SKIP_SOME_PREFIX);
                    encoded.push_str(&msg);
                    encoded
                },
            )
        }
    }
}

/// Generates the step execution body (lookup, validation, execution, result handling).
///
/// This shared implementation is used by both sync and async step executors.
/// The only difference is the function name in the generated code.
fn generate_step_executor_body(path: &TokenStream2) -> TokenStream2 {
    quote! {
        let step = #path::find_step_with_metadata(keyword, #path::StepText::from(text))
            .unwrap_or_else(|| {
                panic!(
                    "Step not found at index {}: {} {} (feature: {}, scenario: {})",
                    index,
                    keyword.as_str(),
                    text,
                    feature_path,
                    scenario_name
                )
            });

        validate_required_fixtures(&step, ctx, text, feature_path, scenario_name);

        match (step.run)(ctx, text, docstring, table) {
            Ok(#path::StepExecution::Skipped { message }) => Err(encode_skip_message(message)),
            Ok(#path::StepExecution::Continue { value }) => Ok(value),
            Err(err) => {
                panic!(
                    "Step failed at index {}: {} {} - {}\n(feature: {}, scenario: {})",
                    index,
                    keyword.as_str(),
                    text,
                    err,
                    feature_path,
                    scenario_name
                );
            }
        }
    }
}

/// Generates a step executor function with the given name.
///
/// This shared implementation is used by both sync and async step executor generators.
/// Both executors have identical logic—the async variant is named differently for
/// clarity in generated code but calls the sync step handler directly.
fn generate_step_executor_impl(fn_name: &str) -> TokenStream2 {
    let path = crate::codegen::rstest_bdd_path();
    let validate_fixtures = generate_validate_fixtures_fn();
    let encode_skip = generate_encode_skip_fn();
    let body = generate_step_executor_body(&path);
    let fn_ident = syn::Ident::new(fn_name, proc_macro2::Span::call_site());

    quote! {
        #[expect(
            clippy::too_many_arguments,
            reason = "helper mirrors generated step inputs to keep panic messaging intact",
        )]
        fn #fn_ident(
            index: usize,
            keyword: #path::StepKeyword,
            text: &str,
            docstring: Option<&str>,
            table: Option<&[&[&str]]>,
            ctx: &mut #path::StepContext,
            feature_path: &str,
            scenario_name: &str,
        ) -> Result<Option<Box<dyn std::any::Any>>, String> {
            #validate_fixtures
            #encode_skip
            #body
        }
    }
}

/// Generates the `__rstest_bdd_execute_single_step` function that looks up
/// and runs a step, handling fixture validation and skip encoding.
///
/// This is the main step execution function that:
/// 1. Looks up the step in the registry using `find_step_with_metadata`
/// 2. Validates required fixtures are available
/// 3. Executes the step and handles the result
/// 4. Encodes skip messages for propagation
///
/// # Generated code
///
/// ```text
/// fn __rstest_bdd_execute_single_step(
///     index, keyword, text, docstring, table, ctx, feature_path, scenario_name
/// ) -> Result<Option<Box<dyn Any>>, String> {
///     // Inner helper functions
///     // Step lookup and execution
///     // Skip handling and error propagation
/// }
/// ```
pub(in crate::codegen::scenario::runtime) fn generate_step_executor() -> TokenStream2 {
    generate_step_executor_impl("__rstest_bdd_execute_single_step")
}

/// Generates the `__rstest_bdd_decode_skip_message` function that decodes
/// skip messages from their encoded format.
///
/// The generated function reverses the encoding done by `encode_skip_message`,
/// extracting the original message from the prefixed format.
///
/// # Usage
///
/// ```ignore
/// let decoder_tokens = generate_skip_decoder();
/// // decoder_tokens is embedded into the scenario test function body
/// ```
///
/// # Generated code
///
/// ```text
/// fn __rstest_bdd_decode_skip_message(encoded: String) -> Option<String> {
///     match encoded.chars().next() {
///         Some(c) if c == __RSTEST_BDD_SKIP_NONE_PREFIX => None,
///         Some(c) if c == __RSTEST_BDD_SKIP_SOME_PREFIX => Some(message),
///         _ => Some(encoded),
///     }
/// }
/// ```
pub(in crate::codegen::scenario::runtime) fn generate_skip_decoder() -> TokenStream2 {
    // The match arms for SKIP_NONE_PREFIX and SKIP_SOME_PREFIX cover all strings
    // produced by encode_skip_message, which always prepends a known prefix.
    // The `_ => Some(encoded)` fallback is a defensive guard for unexpected inputs
    // (e.g., empty string or missing prefix). Rather than panicking, it returns
    // the original encoded string to surface the anomaly without crashing.
    quote! {
        fn __rstest_bdd_decode_skip_message(encoded: String) -> Option<String> {
            match encoded.chars().next() {
                Some(c) if c == __RSTEST_BDD_SKIP_NONE_PREFIX => None,
                Some(c) if c == __RSTEST_BDD_SKIP_SOME_PREFIX => {
                    let prefix_len = c.len_utf8();
                    Some(encoded[prefix_len..].to_string())
                }
                // Defensive: preserve unexpected/malformed input rather than panic
                _ => Some(encoded),
            }
        }
    }
}

/// Generates the `__rstest_bdd_process_async_step` helper function for async step execution.
///
/// The generated function encapsulates step lookup, fixture validation, step execution,
/// and result handling. This mirrors `__rstest_bdd_execute_single_step` for sync execution
/// but is designed for use in async contexts.
///
/// Note: The function itself is not async—it calls the sync step handler directly to avoid
/// higher-ranked trait bound (HRTB) lifetime issues with `AsyncStepFn`. This allows the
/// async executor loop to remain simple while still supporting async test functions.
///
/// # Generated code
///
/// ```text
/// fn __rstest_bdd_process_async_step(
///     index: usize,
///     keyword: StepKeyword,
///     text: &str,
///     docstring: Option<&str>,
///     table: Option<&[&[&str]]>,
///     ctx: &mut StepContext,
///     feature_path: &str,
///     scenario_name: &str,
/// ) -> Result<Option<Box<dyn std::any::Any>>, String> { ... }
/// ```
pub(in crate::codegen::scenario::runtime) fn generate_async_step_executor() -> TokenStream2 {
    generate_step_executor_impl("__rstest_bdd_process_async_step")
}
