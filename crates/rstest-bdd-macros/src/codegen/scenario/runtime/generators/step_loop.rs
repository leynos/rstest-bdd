//! Step executor loop generators.
//!
//! This module emits the loop bodies that iterate through scenario steps and
//! dispatch each step through the appropriate executor. The async and sync
//! variants share the same structure, differing only in the executor callee.

use proc_macro2::TokenStream as TokenStream2;
use quote::quote;

/// Parameter object grouping step data slices for code generation.
///
/// This struct bundles the four parallel arrays of step metadata (keywords,
/// values, docstrings, tables) that are used together during step executor
/// loop generation.
#[derive(Clone, Copy)]
struct StepDataSlices<'a> {
    keyword_tokens: &'a [TokenStream2],
    values: &'a [TokenStream2],
    docstrings: &'a [TokenStream2],
    tables: &'a [TokenStream2],
}

/// Generates the result-handling match block for step execution loops.
///
/// This shared implementation is used by both regular and outline executor loops to
/// handle step execution results: value insertion on success, skip propagation on skip,
/// and deferred panic on actual errors. Errors are stored and the loop breaks, with
/// the panic occurring after the loop completes to mirror the skip-handling pattern.
pub(super) fn generate_step_result_handler(callee: &TokenStream2) -> TokenStream2 {
    quote! {
        match #callee(
            __rstest_bdd_index,
            __rstest_bdd_keyword,
            __rstest_bdd_text,
            __rstest_bdd_docstring,
            __rstest_bdd_table,
            &mut ctx,
            __RSTEST_BDD_FEATURE_PATH,
            __RSTEST_BDD_SCENARIO_NAME,
        ) {
            Ok(Some(__rstest_bdd_val)) => {
                // Intentionally discarded: insert_value returns None when no fixture
                // slot matches the value's TypeId or when matches are ambiguous.
                let _ = ctx.insert_value(__rstest_bdd_val);
            }
            Ok(None) => {}
            Err(ref __rstest_bdd_error) => {
                // Check if this is a skip signal or an actual error
                if let Some(__rstest_bdd_skip_msg) =
                    __rstest_bdd_extract_skip_message(__rstest_bdd_error)
                {
                    __rstest_bdd_skipped = Some(__rstest_bdd_skip_msg);
                    __rstest_bdd_skipped_at = Some(__rstest_bdd_index);
                    break;
                } else {
                    // Store non-skip errors for deferred panic after loop
                    __rstest_bdd_failed = Some(format!("{}", __rstest_bdd_error));
                    break;
                }
            }
        }
    }
}

/// Generates a step executor loop that iterates over steps and handles results.
///
/// This is a shared implementation used by both sync and async executor loop generators.
/// The `callee` parameter is a token stream containing the executor function identifier.
///
/// The generated code initializes tracking variables for both skip and failure states,
/// executes the step loop, and defers any panic until after the loop completes. This
/// mirrors the skip-handling pattern and allows for future enhancement of error context.
fn generate_step_executor_loop_impl(
    callee: &TokenStream2,
    step_data: StepDataSlices<'_>,
) -> TokenStream2 {
    let StepDataSlices {
        keyword_tokens,
        values,
        docstrings,
        tables,
    } = step_data;
    let result_handler = generate_step_result_handler(callee);

    quote! {
        let mut __rstest_bdd_failed: Option<String> = None;
        let __rstest_bdd_steps = [#((#keyword_tokens, #values, #docstrings, #tables)),*];
        for (__rstest_bdd_index, (__rstest_bdd_keyword, __rstest_bdd_text, __rstest_bdd_docstring, __rstest_bdd_table)) in __rstest_bdd_steps.iter().copied().enumerate() {
            #result_handler
        }
        if let Some(error_msg) = __rstest_bdd_failed {
            panic!("{}", error_msg);
        }
    }
}

/// Macro to generate step executor loop functions that differ only in the
/// executor callee. This macro eliminates duplication between async and sync
/// variants whilst preserving distinct public APIs and documentation.
macro_rules! generate_step_executor_loop_fn {
    ($(#[$meta:meta])* $vis:vis fn $fn_name:ident => $executor_ident:ident) => {
        $(#[$meta])*
        $vis fn $fn_name(
            keyword_tokens: &[TokenStream2],
            values: &[TokenStream2],
            docstrings: &[TokenStream2],
            tables: &[TokenStream2],
        ) -> TokenStream2 {
            let callee = quote! { $executor_ident };
            generate_step_executor_loop_impl(
                &callee,
                StepDataSlices {
                    keyword_tokens,
                    values,
                    docstrings,
                    tables,
                },
            )
        }
    };
}

generate_step_executor_loop_fn! {
    /// Generates the async step executor loop that iterates over steps and awaits each.
    ///
    /// The generated code iterates through all scenario steps, executing each one
    /// and handling the results. On success, values are inserted into the context.
    /// On skip, the loop breaks and records the skip position.
    ///
    /// This implementation uses the sync `run` function directly rather than
    /// `run_async`. This avoids higher-ranked trait bound (HRTB) lifetime issues since
    /// sync steps don't create futures that hold borrows across `.await` points.
    /// For scenarios using actual async step definitions (future work), a different
    /// approach will be needed.
    ///
    /// # Usage
    ///
    /// ```ignore
    /// let loop_tokens = generate_async_step_executor_loop(&keywords, &values, &docstrings, &tables);
    /// // loop_tokens is embedded into the async scenario test function body
    /// ```
    pub(in crate::codegen::scenario::runtime) fn generate_async_step_executor_loop => __rstest_bdd_process_async_step
}

generate_step_executor_loop_fn! {
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
    /// let mut __rstest_bdd_failed: Option<String> = None;
    /// let __rstest_bdd_steps = [(keyword, text, docstring, table), ...];
    /// for (index, (keyword, text, docstring, table)) in steps.iter().enumerate() {
    ///     match __rstest_bdd_execute_single_step(...) {
    ///         Ok(value) => { /* insert value into context */ }
    ///         Err(error) => { /* extract skip or store error, break */ }
    ///     }
    /// }
    /// if let Some(error_msg) = __rstest_bdd_failed {
    ///     panic!("{}", error_msg);
    /// }
    /// ```
    pub(in crate::codegen::scenario::runtime) fn generate_step_executor_loop => __rstest_bdd_execute_single_step
}
