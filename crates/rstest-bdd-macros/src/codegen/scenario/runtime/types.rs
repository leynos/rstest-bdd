//! Shared data structures used by the runtime code generator.

use proc_macro2::TokenStream as TokenStream2;

use crate::codegen::scenario::{FeaturePath, ScenarioName};

/// Grouped tokens for scenario steps.
#[derive(Debug)]
pub(crate) struct ProcessedSteps {
    pub(crate) keyword_tokens: Vec<TokenStream2>,
    pub(crate) values: Vec<TokenStream2>,
    pub(crate) docstrings: Vec<TokenStream2>,
    pub(crate) tables: Vec<TokenStream2>,
}

/// Configuration for generating test tokens.
#[derive(Debug)]
pub(crate) struct TestTokensConfig<'a> {
    pub(crate) processed_steps: ProcessedSteps,
    pub(crate) feature_path: &'a FeaturePath,
    pub(crate) scenario_name: &'a ScenarioName,
    pub(crate) scenario_line: u32,
    pub(crate) tags: &'a [String],
    pub(crate) block: &'a syn::Block,
    pub(crate) allow_skipped: bool,
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
pub(super) struct ScenarioLiteralsInput<'a> {
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
