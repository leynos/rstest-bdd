//! Step execution code generators.
//!
//! This module generates the runtime Rust code responsible for executing individual
//! scenario steps. The generators produce thin wrapper functions that delegate to
//! [`rstest_bdd::execution`] for the actual step execution logic.
//!
//! # Responsibilities
//!
//! - Generate `__rstest_bdd_execute_single_step`: thin wrapper calling runtime execution
//! - Generate `__rstest_bdd_process_async_step`: async variant (currently identical to sync)
//!
//! # Design
//!
//! The step execution logic has been moved to [`rstest_bdd::execution`] to achieve
//! clearer separation between macro codegen and runtime policy. The generated code
//! simply delegates to the runtime functions, keeping macro output minimal and
//! centralizing policy decisions in the runtime crate.

use proc_macro2::TokenStream as TokenStream2;
use quote::quote;

/// Generates the `__rstest_bdd_execute_single_step` function.
///
/// The generated function is a thin wrapper that delegates to
/// [`rstest_bdd::execution::execute_step`] for the actual execution logic.
///
/// # Generated code
///
/// ```text
/// fn __rstest_bdd_execute_single_step(
///     index, keyword, text, docstring, table, ctx, feature_path, scenario_name
/// ) -> Result<Option<Box<dyn Any>>, ExecutionError> {
///     rstest_bdd::execution::execute_step(&StepExecutionRequest { ... }, ctx)
/// }
/// ```
pub(in crate::codegen::scenario::runtime) fn generate_step_executor() -> TokenStream2 {
    let path = crate::codegen::rstest_bdd_path();
    quote! {
        #[expect(
            clippy::too_many_arguments,
            reason = "wrapper bridges macro-generated calls to runtime StepExecutionRequest",
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
        ) -> Result<Option<Box<dyn std::any::Any>>, #path::execution::ExecutionError> {
            #path::execution::execute_step(
                &#path::execution::StepExecutionRequest {
                    index,
                    keyword,
                    text,
                    docstring,
                    table,
                    feature_path,
                    scenario_name,
                },
                ctx,
            )
        }
    }
}

/// Generates the `__rstest_bdd_extract_skip_message` function.
///
/// The generated function extracts the skip message from an [`rstest_bdd::execution::ExecutionError::Skip`]
/// variant, returning `Some(Option<String>)` for skips and `None` for other errors.
///
/// # Generated code
///
/// ```text
/// fn __rstest_bdd_extract_skip_message(
///     error: &ExecutionError
/// ) -> Option<Option<String>> {
///     if error.is_skip() {
///         Some(error.skip_message().map(String::from))
///     } else {
///         None
///     }
/// }
/// ```
pub(in crate::codegen::scenario::runtime) fn generate_skip_extractor() -> TokenStream2 {
    let path = crate::codegen::rstest_bdd_path();
    quote! {
        fn __rstest_bdd_extract_skip_message(
            error: &#path::execution::ExecutionError,
        ) -> Option<Option<String>> {
            if error.is_skip() {
                Some(error.skip_message().map(String::from))
            } else {
                None
            }
        }
    }
}

/// Generates the `__rstest_bdd_process_async_step` helper function for async step execution.
///
/// The generated function is a thin wrapper that delegates to
/// [`rstest_bdd::execution::execute_step`]. It mirrors `__rstest_bdd_execute_single_step`
/// but with a different name for use in async contexts.
///
/// Note: The function itself is not async—it calls the sync step handler directly to avoid
/// higher-ranked trait bound (HRTB) lifetime issues with `AsyncStepFn`. This allows the
/// async executor loop to remain simple while still supporting async test functions.
///
/// # Generated code
///
/// ```text
/// fn __rstest_bdd_process_async_step(
///     index, keyword, text, docstring, table, ctx, feature_path, scenario_name
/// ) -> Result<Option<Box<dyn std::any::Any>>, ExecutionError> {
///     rstest_bdd::execution::execute_step(&StepExecutionRequest { ... }, ctx)
/// }
/// ```
pub(in crate::codegen::scenario::runtime) fn generate_async_step_executor() -> TokenStream2 {
    let path = crate::codegen::rstest_bdd_path();
    quote! {
        #[expect(
            clippy::too_many_arguments,
            reason = "wrapper bridges macro-generated calls to runtime StepExecutionRequest",
        )]
        fn __rstest_bdd_process_async_step(
            index: usize,
            keyword: #path::StepKeyword,
            text: &str,
            docstring: Option<&str>,
            table: Option<&[&[&str]]>,
            ctx: &mut #path::StepContext,
            feature_path: &str,
            scenario_name: &str,
        ) -> Result<Option<Box<dyn std::any::Any>>, #path::execution::ExecutionError> {
            #path::execution::execute_step(
                &#path::execution::StepExecutionRequest {
                    index,
                    keyword,
                    text,
                    docstring,
                    table,
                    feature_path,
                    scenario_name,
                },
                ctx,
            )
        }
    }
}
