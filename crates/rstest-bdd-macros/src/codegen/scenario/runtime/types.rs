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

#[derive(Clone, Copy)]
pub(super) struct ScenarioLiteralsInput<'a> {
    pub(super) feature_path: &'a FeaturePath,
    pub(super) scenario_name: &'a ScenarioName,
    pub(super) scenario_line: u32,
    pub(super) tags: &'a [String],
    pub(super) allow_skipped: bool,
}

pub(super) struct CodeComponents {
    pub(super) step_executor: TokenStream2,
    pub(super) skip_decoder: TokenStream2,
    pub(super) scenario_guard: TokenStream2,
    pub(super) step_executor_loop: TokenStream2,
    pub(super) skip_handler: TokenStream2,
}

#[derive(Clone, Copy)]
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
