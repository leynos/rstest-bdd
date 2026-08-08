//! Final token assembly for generated scenario test bodies.
//!
//! Splits the per-scenario constants from the runtime scaffolding so neither
//! helper grows past the module and complexity ceilings enforced in CI.

use proc_macro2::TokenStream as TokenStream2;
use quote::quote;

use super::types::{CodeComponents, ScenarioLiterals, TokenAssemblyContext};

/// Emit the per-scenario constants and the lazily built tag list.
fn scenario_constant_tokens(literals: &ScenarioLiterals, path: &TokenStream2) -> TokenStream2 {
    let ScenarioLiterals {
        feature_literal,
        scenario_literal,
        scenario_line_literal,
        tag_literals,
        ..
    } = literals;
    quote! {
        const __RSTEST_BDD_FEATURE_PATH: &str = #feature_literal;
        const __RSTEST_BDD_SCENARIO_NAME: &str = #scenario_literal;
        const __RSTEST_BDD_SCENARIO_LINE: u32 = #scenario_line_literal;
        static __RSTEST_BDD_SCENARIO_TAGS: std::sync::LazyLock<#path::reporting::ScenarioTags> =
            std::sync::LazyLock::new(|| {
                std::sync::Arc::<[String]>::from(vec![#(#tag_literals.to_string()),*])
            });
    }
}

pub(super) fn assemble_test_tokens(
    literals: &ScenarioLiterals,
    components: CodeComponents,
    context: TokenAssemblyContext<'_>,
) -> TokenStream2 {
    let TokenAssemblyContext {
        ctx_prelude,
        ctx_inserts,
        ctx_postlude,
        block,
    } = context;
    let CodeComponents {
        step_executor,
        skip_extractor,
        scenario_guard,
        step_executor_loop,
        skip_handler,
    } = components;

    let path = crate::codegen::rstest_bdd_path();
    let allow_literal = &literals.allow_literal;
    let scenario_constants = scenario_constant_tokens(literals, &path);
    quote! {
        #scenario_constants
        #step_executor
        #skip_extractor
        #scenario_guard

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
