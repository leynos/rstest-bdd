//! Shared data structures used by the runtime code generator.

use proc_macro2::TokenStream as TokenStream2;

use crate::codegen::scenario::{FeaturePath, ScenarioName};

/// Grouped tokens for scenario steps.
pub(crate) struct ProcessedSteps {
    pub(crate) keyword_tokens: Vec<TokenStream2>,
    pub(crate) values: Vec<TokenStream2>,
    pub(crate) docstrings: Vec<TokenStream2>,
    pub(crate) tables: Vec<TokenStream2>,
}

/// Configuration for generating test tokens.
pub(crate) struct TestTokensConfig<'a> {
    pub(crate) processed_steps: ProcessedSteps,
    pub(crate) feature_path: &'a FeaturePath,
    pub(crate) scenario_name: &'a ScenarioName,
    pub(crate) scenario_line: u32,
    pub(crate) tags: &'a [String],
    pub(crate) block: &'a syn::Block,
    pub(crate) allow_skipped: bool,
}

pub(super) struct ScenarioLiterals {
    pub(super) allow_literal: syn::LitBool,
    pub(super) feature_literal: syn::LitStr,
    pub(super) scenario_literal: syn::LitStr,
    pub(super) scenario_line_literal: syn::LitInt,
    pub(super) tag_literals: Vec<syn::LitStr>,
}

pub(super) struct ScenarioLiteralsInput<'a> {
    pub(super) feature_path: &'a FeaturePath,
    pub(super) scenario_name: &'a ScenarioName,
    pub(super) scenario_line: u32,
    pub(super) tags: &'a [String],
    pub(super) allow_skipped: bool,
}

impl<'a> ScenarioLiteralsInput<'a> {
    pub(super) fn new(
        feature_path: &'a FeaturePath,
        scenario_name: &'a ScenarioName,
        scenario_line: u32,
        tags: &'a [String],
        allow_skipped: bool,
    ) -> Self {
        Self {
            feature_path,
            scenario_name,
            scenario_line,
            tags,
            allow_skipped,
        }
    }
}

pub(super) struct CodeComponents {
    pub(super) step_executor: TokenStream2,
    pub(super) skip_decoder: TokenStream2,
    pub(super) scenario_guard: TokenStream2,
    pub(super) step_executor_loop: TokenStream2,
    pub(super) skip_handler: TokenStream2,
}
