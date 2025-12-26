//! Helpers that generate the runtime scaffolding for scenario tests.

mod generators;
#[cfg(test)]
mod tests;
mod types;

use proc_macro2::TokenStream as TokenStream2;
use quote::quote;

use generators::{
    generate_scenario_guard, generate_skip_decoder, generate_skip_handler, generate_step_executor,
    generate_step_executor_loop,
};
use types::{CodeComponents, ScenarioLiterals, ScenarioLiteralsInput, TokenAssemblyContext};
pub(crate) use types::{ProcessedSteps, TestTokensConfig};

fn create_scenario_literals(input: ScenarioLiteralsInput<'_>) -> ScenarioLiterals {
    let allow_literal = syn::LitBool::new(input.allow_skipped, proc_macro2::Span::call_site());
    let feature_literal =
        syn::LitStr::new(input.feature_path.as_str(), proc_macro2::Span::call_site());
    let scenario_literal =
        syn::LitStr::new(input.scenario_name.as_str(), proc_macro2::Span::call_site());
    let scenario_line_literal = syn::LitInt::new(
        &input.scenario_line.to_string(),
        proc_macro2::Span::call_site(),
    );
    let tag_literals = input
        .tags
        .iter()
        .map(|tag| syn::LitStr::new(tag, proc_macro2::Span::call_site()))
        .collect();

    ScenarioLiterals {
        allow_literal,
        feature_literal,
        scenario_literal,
        scenario_line_literal,
        tag_literals,
    }
}

fn generate_code_components(processed_steps: &ProcessedSteps) -> CodeComponents {
    let ProcessedSteps {
        keyword_tokens,
        values,
        docstrings,
        tables,
    } = processed_steps;

    let step_executor = generate_step_executor();
    let skip_decoder = generate_skip_decoder();
    let scenario_guard = generate_scenario_guard();
    let step_executor_loop =
        generate_step_executor_loop(keyword_tokens, values, docstrings, tables);
    let skip_handler = generate_skip_handler();

    CodeComponents {
        step_executor,
        skip_decoder,
        scenario_guard,
        step_executor_loop,
        skip_handler,
    }
}

pub(crate) fn generate_test_tokens(
    config: TestTokensConfig<'_>,
    ctx_prelude: impl Iterator<Item = TokenStream2>,
    ctx_inserts: impl Iterator<Item = TokenStream2>,
    ctx_postlude: impl Iterator<Item = TokenStream2>,
) -> TokenStream2 {
    let TestTokensConfig {
        processed_steps,
        feature_path,
        scenario_name,
        scenario_line,
        tags,
        block,
        allow_skipped,
    } = config;
    let ctx_prelude: Vec<_> = ctx_prelude.collect();
    let ctx_inserts: Vec<_> = ctx_inserts.collect();
    let ctx_postlude: Vec<_> = ctx_postlude.collect();

    let literals = create_scenario_literals(ScenarioLiteralsInput {
        feature_path,
        scenario_name,
        scenario_line,
        tags,
        allow_skipped,
    });

    let components = generate_code_components(&processed_steps);
    let block_tokens = quote! { #block };
    let context =
        TokenAssemblyContext::new(&ctx_prelude, &ctx_inserts, &ctx_postlude, &block_tokens);

    assemble_test_tokens(literals, components, context)
}

fn assemble_test_tokens(
    literals: ScenarioLiterals,
    components: CodeComponents,
    context: TokenAssemblyContext<'_>,
) -> TokenStream2 {
    let TokenAssemblyContext {
        ctx_prelude,
        ctx_inserts,
        ctx_postlude,
        block,
    } = context;
    let ScenarioLiterals {
        allow_literal,
        feature_literal,
        scenario_literal,
        scenario_line_literal,
        tag_literals,
    } = literals;

    let CodeComponents {
        step_executor,
        skip_decoder,
        scenario_guard,
        step_executor_loop,
        skip_handler,
    } = components;

    let path = crate::codegen::rstest_bdd_path();
    quote! {
        const __RSTEST_BDD_FEATURE_PATH: &str = #feature_literal;
        const __RSTEST_BDD_SCENARIO_NAME: &str = #scenario_literal;
        const __RSTEST_BDD_SCENARIO_LINE: u32 = #scenario_line_literal;
        static __RSTEST_BDD_SCENARIO_TAGS: std::sync::LazyLock<#path::reporting::ScenarioTags> =
            std::sync::LazyLock::new(|| {
                std::sync::Arc::<[String]>::from(vec![#(#tag_literals.to_string()),*])
            });
        const SKIP_NONE_PREFIX: char = '\u{0}';
        const SKIP_SOME_PREFIX: char = '\u{1}';
        #step_executor
        #skip_decoder
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
