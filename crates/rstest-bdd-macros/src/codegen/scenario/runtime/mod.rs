//! Helpers that generate the runtime scaffolding for scenario tests.

mod assembly;
mod body;
mod generators;
mod harness;
#[cfg(test)]
mod tests;
mod types;

use assembly::assemble_test_tokens;
use body::wrap_scenario_block;
use generators::{
    generate_async_step_executor,
    generate_async_step_executor_loop,
    generate_async_step_executor_loop_outline,
    generate_scenario_guard,
    generate_skip_extractor,
    generate_skip_handler,
    generate_step_executor,
    generate_step_executor_loop,
    generate_step_executor_loop_outline,
};
use harness::assemble_test_tokens_with_harness;
use proc_macro2::TokenStream as TokenStream2;
use types::{CodeComponents, ScenarioLiterals, ScenarioLiteralsInput, TokenAssemblyContext};
pub(crate) use types::{ProcessedSteps, ScenarioMetadata, TestTokensConfig};

use super::helpers::ProcessedStepTokens;
use crate::codegen::scenario::ScenarioReturnKind;

/// Configuration for generating test tokens for scenario outlines.
#[derive(Debug)]
pub(crate) struct OutlineTestTokensConfig<'a> {
    /// Processed steps for each Examples row (one set per row).
    pub(crate) all_rows_steps: Vec<ProcessedStepTokens>,
    pub(crate) metadata: ScenarioMetadata<'a>,
}

/// Common interface for scenario test configuration types.
trait ScenarioTestConfig {
    /// Generates the code components for this scenario type.
    fn generate_components(&self) -> CodeComponents;

    /// Extracts the common scenario metadata fields.
    fn literals_input(&self) -> ScenarioLiteralsInput<'_>;

    /// Returns the test function block.
    fn block(&self) -> &syn::Block;

    /// Returns the return kind for the scenario body.
    fn return_kind(&self) -> ScenarioReturnKind;

    /// Whether the scenario runs asynchronously.
    fn is_async(&self) -> bool;

    /// Returns the optional harness adapter type path for execution delegation.
    fn harness(&self) -> Option<&syn::Path>;
}

impl ScenarioTestConfig for TestTokensConfig<'_> {
    fn generate_components(&self) -> CodeComponents {
        generate_code_components(
            &self.processed_steps,
            self.metadata.is_async,
            self.metadata.return_kind,
        )
    }

    fn literals_input(&self) -> ScenarioLiteralsInput<'_> { self.metadata.literals_input() }

    fn block(&self) -> &syn::Block { self.metadata.block }

    fn return_kind(&self) -> ScenarioReturnKind { self.metadata.return_kind }

    fn is_async(&self) -> bool { self.metadata.is_async }

    fn harness(&self) -> Option<&syn::Path> { self.metadata.harness }
}

impl ScenarioTestConfig for OutlineTestTokensConfig<'_> {
    fn generate_components(&self) -> CodeComponents {
        generate_code_components_outline(
            &self.all_rows_steps,
            self.metadata.is_async,
            self.metadata.return_kind,
        )
    }

    fn literals_input(&self) -> ScenarioLiteralsInput<'_> { self.metadata.literals_input() }

    fn block(&self) -> &syn::Block { self.metadata.block }

    fn return_kind(&self) -> ScenarioReturnKind { self.metadata.return_kind }

    fn is_async(&self) -> bool { self.metadata.is_async }

    fn harness(&self) -> Option<&syn::Path> { self.metadata.harness }
}

/// Context token stream iterators for test generation.
pub(super) struct ContextIterators<P, I, Q>
where
    P: Iterator<Item = TokenStream2>,
    I: Iterator<Item = TokenStream2>,
    Q: Iterator<Item = TokenStream2>,
{
    pub prelude: P,
    pub inserts: I,
    pub postlude: Q,
}

impl<P, I, Q> ContextIterators<P, I, Q>
where
    P: Iterator<Item = TokenStream2>,
    I: Iterator<Item = TokenStream2>,
    Q: Iterator<Item = TokenStream2>,
{
    pub const fn new(prelude: P, inserts: I, postlude: Q) -> Self {
        Self {
            prelude,
            inserts,
            postlude,
        }
    }
}

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

/// Returns the common runtime components shared by regular and outline scenarios.
///
/// Extracts `step_executor`, `skip_extractor`, `scenario_guard`, and `skip_handler` to avoid
/// duplicating these generators in both `generate_code_components` and
/// `generate_code_components_outline`.
fn generate_common_components(
    is_async: bool,
    return_kind: ScenarioReturnKind,
) -> (TokenStream2, TokenStream2, TokenStream2, TokenStream2) {
    let step_executor = if is_async {
        generate_async_step_executor()
    } else {
        generate_step_executor()
    };

    (
        step_executor,
        generate_skip_extractor(),
        generate_scenario_guard(),
        generate_skip_handler(return_kind),
    )
}

fn generate_code_components(
    processed_steps: &ProcessedSteps,
    is_async: bool,
    return_kind: ScenarioReturnKind,
) -> CodeComponents {
    let (step_executor, skip_extractor, scenario_guard, skip_handler) =
        generate_common_components(is_async, return_kind);
    let ProcessedSteps {
        keyword_tokens,
        values,
        docstrings,
        tables,
    } = processed_steps;

    let step_executor_loop = if is_async {
        generate_async_step_executor_loop(keyword_tokens, values, docstrings, tables)
    } else {
        generate_step_executor_loop(keyword_tokens, values, docstrings, tables)
    };

    CodeComponents {
        step_executor,
        skip_extractor,
        scenario_guard,
        step_executor_loop,
        skip_handler,
    }
}

pub(crate) fn generate_test_tokens(
    config: &TestTokensConfig<'_>,
    ctx_prelude: impl Iterator<Item = TokenStream2>,
    ctx_inserts: impl Iterator<Item = TokenStream2>,
    ctx_postlude: impl Iterator<Item = TokenStream2>,
) -> TokenStream2 {
    generate_test_tokens_for_config(
        config,
        ContextIterators::new(ctx_prelude, ctx_inserts, ctx_postlude),
    )
}

/// Assembles test tokens using the provided components and configuration.
///
/// This helper consolidates the common pipeline shared by regular and outline
/// test token generation: collecting context iterators, creating literals,
/// and assembling the final token stream. When a harness adapter is specified,
/// delegates to `assemble_test_tokens_with_harness` instead.
/// The scenario body and the shape it is wrapped in.
#[derive(Clone, Copy)]
struct ScenarioBody<'a> {
    literals_input: ScenarioLiteralsInput<'a>,
    block: &'a syn::Block,
    return_kind: ScenarioReturnKind,
    is_async: bool,
    harness: Option<&'a syn::Path>,
}

fn assemble_test_tokens_with_context<P, I, Q>(
    body: ScenarioBody<'_>,
    components: CodeComponents,
    ctx_iterators: ContextIterators<P, I, Q>,
) -> TokenStream2
where
    P: Iterator<Item = TokenStream2>,
    I: Iterator<Item = TokenStream2>,
    Q: Iterator<Item = TokenStream2>,
{
    let ScenarioBody {
        literals_input,
        block,
        return_kind,
        is_async,
        harness,
    } = body;
    let ctx_prelude: Vec<_> = ctx_iterators.prelude.collect();
    let ctx_inserts: Vec<_> = ctx_iterators.inserts.collect();
    let ctx_postlude: Vec<_> = ctx_iterators.postlude.collect();

    let literals = create_scenario_literals(literals_input);

    let block_tokens = wrap_scenario_block(block, return_kind, is_async);
    let context =
        TokenAssemblyContext::new(&ctx_prelude, &ctx_inserts, &ctx_postlude, &block_tokens);

    if let Some(harness_path) = harness {
        assemble_test_tokens_with_harness(&literals, &components, context, harness_path)
    } else {
        assemble_test_tokens(&literals, components, context)
    }
}

/// Generates test tokens for any scenario configuration.
fn generate_test_tokens_for_config<P, I, Q>(
    config: &impl ScenarioTestConfig,
    ctx_iterators: ContextIterators<P, I, Q>,
) -> TokenStream2
where
    P: Iterator<Item = TokenStream2>,
    I: Iterator<Item = TokenStream2>,
    Q: Iterator<Item = TokenStream2>,
{
    let components = config.generate_components();
    assemble_test_tokens_with_context(
        ScenarioBody {
            literals_input: config.literals_input(),
            block: config.block(),
            return_kind: config.return_kind(),
            is_async: config.is_async(),
            harness: config.harness(),
        },
        components,
        ctx_iterators,
    )
}

/// Generates code components for scenario outlines with per-row step substitution.
///
/// Unlike `generate_code_components`, which handles a single step array,
/// this function accepts a 2D array of processed steps (one set per Examples row)
/// and produces a loop executor that iterates over rows at runtime.
fn generate_code_components_outline(
    all_rows_steps: &[ProcessedStepTokens],
    is_async: bool,
    return_kind: ScenarioReturnKind,
) -> CodeComponents {
    let (step_executor, skip_extractor, scenario_guard, skip_handler) =
        generate_common_components(is_async, return_kind);
    let step_executor_loop = if is_async {
        generate_async_step_executor_loop_outline(all_rows_steps)
    } else {
        generate_step_executor_loop_outline(all_rows_steps)
    };

    CodeComponents {
        step_executor,
        skip_extractor,
        scenario_guard,
        step_executor_loop,
        skip_handler,
    }
}

/// Generates test tokens for scenario outlines with placeholder substitution.
///
/// This function creates the test body for scenario outlines where step text
/// contains placeholders that are substituted with values from the Examples table.
/// Each Examples row produces a separate test case, and the substituted steps
/// are organized in a 2D array indexed by case.
pub(crate) fn generate_test_tokens_outline(
    config: &OutlineTestTokensConfig<'_>,
    ctx_prelude: impl Iterator<Item = TokenStream2>,
    ctx_inserts: impl Iterator<Item = TokenStream2>,
    ctx_postlude: impl Iterator<Item = TokenStream2>,
) -> TokenStream2 {
    generate_test_tokens_for_config(
        config,
        ContextIterators::new(ctx_prelude, ctx_inserts, ctx_postlude),
    )
}
