//! Step executor loop generators for scenario outlines.
//!
//! This module generates the loop bodies that execute outline steps with
//! placeholder substitution. Outline steps are emitted as 2D arrays, one row per
//! examples entry, and the executor loop selects the appropriate row before
//! dispatching each step through the shared result handler.

use proc_macro2::TokenStream as TokenStream2;
use quote::quote;

use crate::codegen::scenario::helpers::ProcessedStepTokens;

use super::step_loop::generate_step_result_handler;

/// Generates a step executor loop for scenario outlines with placeholder substitution.
///
/// This shared implementation accepts a `TokenStream2` callee for consistency with
/// `generate_step_executor_loop_impl`. It initializes tracking variables for both skip
/// and failure states, executes the step loop, and defers any panic until after the
/// loop completes.
fn generate_step_executor_loop_outline_impl(
    callee: &TokenStream2,
    all_rows_steps: &[ProcessedStepTokens],
) -> TokenStream2 {
    let path = crate::codegen::rstest_bdd_path();

    let row_arrays: Vec<TokenStream2> = all_rows_steps
        .iter()
        .map(|(keywords, values, docstrings, tables)| {
            quote! {
                &[#((#keywords, #values, #docstrings, #tables)),*]
            }
        })
        .collect();
    let result_handler = generate_step_result_handler(callee);

    quote! {
        let mut __rstest_bdd_failed: Option<String> = None;
        const __RSTEST_BDD_ALL_STEPS: &[&[(#path::StepKeyword, &str, Option<&str>, Option<&[&[&str]]>)]] = &[
            #(#row_arrays),*
        ];
        let __rstest_bdd_steps = __RSTEST_BDD_ALL_STEPS[__rstest_bdd_case_idx];
        for (__rstest_bdd_index, (__rstest_bdd_keyword, __rstest_bdd_text, __rstest_bdd_docstring, __rstest_bdd_table)) in __rstest_bdd_steps.iter().copied().enumerate() {
            #result_handler
        }
        if let Some(error_msg) = __rstest_bdd_failed {
            panic!("{}", error_msg);
        }
    }
}

/// Generates the step executor loop for scenario outlines with placeholder substitution.
///
/// For scenario outlines, steps are organized as a 2D array where each row contains
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
/// let mut __rstest_bdd_failed: Option<String> = None;
/// const __RSTEST_BDD_ALL_STEPS: &[&[(StepKeyword, &str, Option<&str>, Option<&[&[&str]]>)]] = &[
///     &[(kw0, "substituted text row 0", ...), ...],
///     &[(kw0, "substituted text row 1", ...), ...],
/// ];
/// let __rstest_bdd_steps = __RSTEST_BDD_ALL_STEPS[__rstest_bdd_case_idx];
/// for (index, (keyword, text, docstring, table)) in steps.iter().copied().enumerate() {
///     match __rstest_bdd_execute_single_step(...) {
///         Ok(value) => { /* insert value into context */ }
///         Err(error) => { /* extract skip or store error, break */ }
///     }
/// }
/// if let Some(error_msg) = __rstest_bdd_failed {
///     panic!("{}", error_msg);
/// }
/// ```
pub(in crate::codegen::scenario::runtime) fn generate_step_executor_loop_outline(
    all_rows_steps: &[ProcessedStepTokens],
) -> TokenStream2 {
    let callee = quote! { __rstest_bdd_execute_single_step };
    generate_step_executor_loop_outline_impl(&callee, all_rows_steps)
}

/// Generates the async step executor loop for scenario outlines.
///
/// For scenario outlines, steps are organized as a 2D array where each row contains
/// the substituted steps for one Examples row. The `__rstest_bdd_case_idx` parameter
/// selects which row to use. This async variant dispatches each substituted step
/// through the `__rstest_bdd_process_async_step` helper.
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
///     match __rstest_bdd_process_async_step(...) {
///         Ok(value) => { /* insert value into context */ }
///         Err(error) => { /* extract skip or store error, break */ }
///     }
/// }
/// if let Some(error_msg) = __rstest_bdd_failed {
///     panic!("{}", error_msg);
/// }
/// ```
///
/// # Usage
///
/// This generator is used by async scenario outline code generation and ensures
/// async test functions execute the substituted steps in the correct order.
pub(in crate::codegen::scenario::runtime) fn generate_async_step_executor_loop_outline(
    all_rows_steps: &[ProcessedStepTokens],
) -> TokenStream2 {
    let callee = quote! { __rstest_bdd_process_async_step };
    generate_step_executor_loop_outline_impl(&callee, all_rows_steps)
}
