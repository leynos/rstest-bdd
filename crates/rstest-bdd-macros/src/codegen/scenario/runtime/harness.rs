//! Token assembly for harness-delegated scenario execution.
//!
//! When a harness adapter type is specified via the `harness` parameter, the
//! runtime portion of the test (context setup, step executor loop, skip handler,
//! postlude, and user block) is wrapped in a closure passed to
//! `HarnessAdapter::run()`. Item definitions (constants, inner functions,
//! structs) remain outside the closure because they are Rust items visible by
//! name resolution, not captured variables.

use proc_macro2::TokenStream as TokenStream2;
use quote::quote;

use super::types::{CodeComponents, ScenarioLiterals, TokenAssemblyContext};

/// Generates the const/static metadata declarations and item definitions
/// (step executor, skip extractor, scenario guard) that live outside the
/// runner closure.
fn generate_metadata_constants(
    literals: &ScenarioLiterals,
    components: &CodeComponents,
    path: &TokenStream2,
) -> TokenStream2 {
    let ScenarioLiterals {
        feature_literal,
        scenario_literal,
        scenario_line_literal,
        tag_literals,
        ..
    } = literals;
    let CodeComponents {
        step_executor,
        skip_extractor,
        scenario_guard,
        ..
    } = components;

    quote! {
        const __RSTEST_BDD_FEATURE_PATH: &str = #feature_literal;
        const __RSTEST_BDD_SCENARIO_NAME: &str = #scenario_literal;
        const __RSTEST_BDD_SCENARIO_LINE: u32 = #scenario_line_literal;
        static __RSTEST_BDD_SCENARIO_TAGS: std::sync::LazyLock<#path::reporting::ScenarioTags> =
            std::sync::LazyLock::new(|| {
                std::sync::Arc::<[String]>::from(vec![#(#tag_literals.to_string()),*])
            });

        #step_executor
        #skip_extractor
        #scenario_guard
    }
}

/// Parameters for generating the runner closure body.
#[derive(Clone, Copy)]
struct RunnerClosureParams<'a> {
    allow_literal: &'a syn::LitBool,
    ctx_prelude: &'a [TokenStream2],
    ctx_inserts: &'a [TokenStream2],
    ctx_postlude: &'a [TokenStream2],
    block: &'a TokenStream2,
    step_executor_loop: &'a TokenStream2,
    skip_handler: &'a TokenStream2,
    path: &'a TokenStream2,
}

/// Generates the body of the `ScenarioRunner` closure: context setup, step
/// execution loop, skip handling, postlude, and the user block.
fn generate_runner_closure_body(params: RunnerClosureParams<'_>) -> TokenStream2 {
    let RunnerClosureParams {
        allow_literal,
        ctx_prelude,
        ctx_inserts,
        ctx_postlude,
        block,
        step_executor_loop,
        skip_handler,
        path,
    } = params;

    quote! {
        let __rstest_bdd_allow_skipped: bool = #allow_literal;
        #(#ctx_prelude)*
        let mut ctx = {
            let mut ctx = #path::StepContext::default();
            #(#ctx_inserts)*
            ctx
        };

        let mut __rstest_bdd_scenario_guard = __RstestBddScenarioReportGuard::new(
            __RSTEST_BDD_FEATURE_PATH,
            __RSTEST_BDD_SCENARIO_NAME,
            __RSTEST_BDD_SCENARIO_LINE,
            __RSTEST_BDD_SCENARIO_TAGS.clone(),
        );
        let mut __rstest_bdd_skipped: Option<Option<String>> = None;
        let mut __rstest_bdd_skipped_at: Option<usize> = None;
        #step_executor_loop
        #skip_handler
        #(#ctx_postlude)*
        #block
    }
}

/// Assembles test tokens with harness delegation.
pub(super) fn assemble_test_tokens_with_harness(
    literals: &ScenarioLiterals,
    components: &CodeComponents,
    context: TokenAssemblyContext<'_>,
    harness_path: &syn::Path,
) -> TokenStream2 {
    let path = crate::codegen::rstest_bdd_path();
    let harness_crate = crate::codegen::rstest_bdd_harness_path();

    let constants = generate_metadata_constants(literals, components, &path);
    let closure_body = generate_runner_closure_body(RunnerClosureParams {
        allow_literal: &literals.allow_literal,
        ctx_prelude: context.ctx_prelude,
        ctx_inserts: context.ctx_inserts,
        ctx_postlude: context.ctx_postlude,
        block: context.block,
        step_executor_loop: &components.step_executor_loop,
        skip_handler: &components.skip_handler,
        path: &path,
    });

    let tag_literals = &literals.tag_literals;

    quote! {
        #constants

        let __rstest_bdd_harness_metadata = #harness_crate::ScenarioMetadata::new(
            __RSTEST_BDD_FEATURE_PATH,
            __RSTEST_BDD_SCENARIO_NAME,
            __RSTEST_BDD_SCENARIO_LINE,
            vec![#(#tag_literals.to_string()),*],
        );

        let __rstest_bdd_runner = #harness_crate::ScenarioRunner::new(move || {
            #closure_body
        });

        let __rstest_bdd_request = #harness_crate::ScenarioRunRequest::new(
            __rstest_bdd_harness_metadata,
            __rstest_bdd_runner,
        );

        <#harness_path as Default>::default().run(__rstest_bdd_request)
    }
}
