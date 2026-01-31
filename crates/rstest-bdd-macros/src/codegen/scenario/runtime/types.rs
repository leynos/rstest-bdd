//! Shared data structures used by the runtime code generator.

use proc_macro2::TokenStream as TokenStream2;

use crate::codegen::scenario::{FeaturePath, ScenarioName, ScenarioReturnKind};

/// Grouped tokens for scenario steps.
#[derive(Debug)]
pub(crate) struct ProcessedSteps {
    pub(crate) keyword_tokens: Vec<TokenStream2>,
    pub(crate) values: Vec<TokenStream2>,
    pub(crate) docstrings: Vec<TokenStream2>,
    pub(crate) tables: Vec<TokenStream2>,
}

/// Shared metadata for scenario test generation.
#[derive(Debug, Clone, Copy)]
pub(crate) struct ScenarioMetadata<'a> {
    pub(crate) feature_path: &'a FeaturePath,
    pub(crate) scenario_name: &'a ScenarioName,
    pub(crate) scenario_line: u32,
    pub(crate) tags: &'a [String],
    pub(crate) block: &'a syn::Block,
    pub(crate) allow_skipped: bool,
    /// Whether to generate async step execution code.
    pub(crate) is_async: bool,
    /// Expected return kind for the scenario body.
    pub(crate) return_kind: ScenarioReturnKind,
}

impl<'a> ScenarioMetadata<'a> {
    pub(crate) fn literals_input(&self) -> ScenarioLiteralsInput<'a> {
        ScenarioLiteralsInput {
            feature_path: self.feature_path,
            scenario_name: self.scenario_name,
            scenario_line: self.scenario_line,
            tags: self.tags,
            allow_skipped: self.allow_skipped,
        }
    }
}

/// Configuration for generating test tokens.
#[derive(Debug)]
pub(crate) struct TestTokensConfig<'a> {
    pub(crate) processed_steps: ProcessedSteps,
    pub(crate) metadata: ScenarioMetadata<'a>,
}

#[derive(Debug)]
pub(super) struct ScenarioLiterals {
    pub(super) allow_literal: syn::LitBool,
    pub(super) feature_literal: syn::LitStr,
    pub(super) scenario_literal: syn::LitStr,
    pub(super) scenario_line_literal: syn::LitInt,
    pub(super) tag_literals: Vec<syn::LitStr>,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct ScenarioLiteralsInput<'a> {
    pub(super) feature_path: &'a FeaturePath,
    pub(super) scenario_name: &'a ScenarioName,
    pub(super) scenario_line: u32,
    pub(super) tags: &'a [String],
    pub(super) allow_skipped: bool,
}

#[derive(Debug)]
pub(super) struct CodeComponents {
    pub(super) step_executor: TokenStream2,
    pub(super) skip_decoder: TokenStream2,
    pub(super) scenario_guard: TokenStream2,
    pub(super) step_executor_loop: TokenStream2,
    pub(super) skip_handler: TokenStream2,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct TokenAssemblyContext<'a> {
    pub(super) ctx_prelude: &'a [TokenStream2],
    pub(super) ctx_inserts: &'a [TokenStream2],
    pub(super) ctx_postlude: &'a [TokenStream2],
    pub(super) block: &'a TokenStream2,
}

impl<'a> TokenAssemblyContext<'a> {
    pub(super) fn new(
        ctx_prelude: &'a [TokenStream2],
        ctx_inserts: &'a [TokenStream2],
        ctx_postlude: &'a [TokenStream2],
        block: &'a TokenStream2,
    ) -> Self {
        Self {
            ctx_prelude,
            ctx_inserts,
            ctx_postlude,
            block,
        }
    }
}
