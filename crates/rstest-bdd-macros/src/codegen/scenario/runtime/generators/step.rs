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

use crate::codegen::scenario::helpers::ProcessedStepTokens;

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
    let path = crate::codegen::rstest_bdd_path();
    let validate_fixtures = generate_validate_fixtures_fn();
    let encode_skip = generate_encode_skip_fn();

    quote! {
        #[expect(
            clippy::too_many_arguments,
            reason = "helper mirrors generated step inputs to keep panic messaging intact",
        )]
        fn __rstest_bdd_execute_single_step(
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

            if let Some(step) = #path::find_step_with_metadata(keyword, #path::StepText::from(text)) {
                validate_required_fixtures(&step, ctx, text, feature_path, scenario_name);

                match (step.run)(ctx, text, docstring, table) {
                    Ok(#path::StepExecution::Skipped { message }) => {
                        return Err(encode_skip_message(message));
                    }
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
            } else {
                panic!(
                    "Step not found at index {}: {} {} (feature: {}, scenario: {})",
                    index,
                    keyword.as_str(),
                    text,
                    feature_path,
                    scenario_name
                );
            }
        }
    }
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

/// Generates the step executor loop that iterates over steps and handles results.
///
/// The generated code iterates through all scenario steps, executing each one
/// and handling the results. On success, values are inserted into the context.
/// On skip, the loop breaks and records the skip position.
///
/// # Usage
///
/// ```ignore
/// let loop_tokens = generate_step_executor_loop(&keywords, &values, &docstrings, &tables);
/// // loop_tokens is embedded into the scenario test function body
/// ```
///
/// # Generated code
///
/// ```text
/// let __rstest_bdd_steps = [(keyword, text, docstring, table), ...];
/// for (index, (keyword, text, docstring, table)) in steps.iter().enumerate() {
///     match __rstest_bdd_execute_single_step(...) {
///         Ok(value) => { /* insert value into context */ }
///         Err(encoded) => { /* decode skip, record position, break */ }
///     }
/// }
/// ```
pub(in crate::codegen::scenario::runtime) fn generate_step_executor_loop(
    keyword_tokens: &[TokenStream2],
    values: &[TokenStream2],
    docstrings: &[TokenStream2],
    tables: &[TokenStream2],
) -> TokenStream2 {
    let loop_body = generate_step_executor_loop_body();
    quote! {
        let __rstest_bdd_steps = [#((#keyword_tokens, #values, #docstrings, #tables)),*];
        for (__rstest_bdd_index, (__rstest_bdd_keyword, __rstest_bdd_text, __rstest_bdd_docstring, __rstest_bdd_table)) in __rstest_bdd_steps.iter().copied().enumerate() {
            #loop_body
        }
    }
}

/// Generates the step executor loop for scenario outlines with placeholder substitution.
///
/// For scenario outlines, steps are organised as a 2D array where each row contains
/// the substituted steps for one Examples row. The `__rstest_bdd_case_idx` parameter
/// selects which row to use.
///
/// # Arguments
///
/// * `all_rows_steps` - A vector where each element contains the processed steps for
///   one Examples row. Each inner tuple contains (keywords, values, docstrings, tables).
///
/// # Generated code
///
/// ```text
/// const __RSTEST_BDD_ALL_STEPS: &[&[(StepKeyword, &str, Option<&str>, Option<&[&[&str]]>)]] = &[
///     &[(kw0, "substituted text row 0", ...), ...],
///     &[(kw0, "substituted text row 1", ...), ...],
/// ];
/// let __rstest_bdd_steps = __RSTEST_BDD_ALL_STEPS[__rstest_bdd_case_idx];
/// for (index, (keyword, text, docstring, table)) in steps.iter().copied().enumerate() {
///     // ... same execution logic as regular loop
/// }
/// ```
pub(in crate::codegen::scenario::runtime) fn generate_step_executor_loop_outline(
    all_rows_steps: &[ProcessedStepTokens],
) -> TokenStream2 {
    let path = crate::codegen::rstest_bdd_path();
    let loop_body = generate_step_executor_loop_body();

    // Build the 2D array of steps: one inner array per Examples row
    let row_arrays: Vec<TokenStream2> = all_rows_steps
        .iter()
        .map(|(keywords, values, docstrings, tables)| {
            quote! {
                &[#((#keywords, #values, #docstrings, #tables)),*]
            }
        })
        .collect();

    quote! {
        const __RSTEST_BDD_ALL_STEPS: &[&[(#path::StepKeyword, &str, Option<&str>, Option<&[&[&str]]>)]] = &[
            #(#row_arrays),*
        ];
        let __rstest_bdd_steps = __RSTEST_BDD_ALL_STEPS[__rstest_bdd_case_idx];
        for (__rstest_bdd_index, (__rstest_bdd_keyword, __rstest_bdd_text, __rstest_bdd_docstring, __rstest_bdd_table)) in __rstest_bdd_steps.iter().copied().enumerate() {
            #loop_body
        }
    }
}

fn generate_step_executor_loop_body() -> TokenStream2 {
    quote! {
        match __rstest_bdd_execute_single_step(
            __rstest_bdd_index,
            __rstest_bdd_keyword,
            __rstest_bdd_text,
            __rstest_bdd_docstring,
            __rstest_bdd_table,
            &mut ctx,
            __RSTEST_BDD_FEATURE_PATH,
            __RSTEST_BDD_SCENARIO_NAME,
        ) {
            Ok(__rstest_bdd_value) => {
                if let Some(__rstest_bdd_val) = __rstest_bdd_value {
                    // Intentionally discarded: insert_value returns None when no fixture
                    // slot matches the value's TypeId or when matches are ambiguous.
                    let _ = ctx.insert_value(__rstest_bdd_val);
                }
            }
            Err(__rstest_bdd_encoded) => {
                __rstest_bdd_skipped =
                    Some(__rstest_bdd_decode_skip_message(__rstest_bdd_encoded));
                __rstest_bdd_skipped_at = Some(__rstest_bdd_index);
                break;
            }
        }
    }
}
