//! Shared data structures used by the runtime code generator.

use proc_macro2::TokenStream as TokenStream2;

use crate::codegen::scenario::helpers::ProcessedStepTokens;
use crate::codegen::scenario::{FeaturePath, ScenarioName, ScenarioReturnKind};

/// Grouped tokens for scenario steps.
#[derive(Debug)]
pub(crate) struct ProcessedSteps {
    /// Stores the internal `keyword_tokens` value.
    pub(crate) keyword_tokens: Vec<TokenStream2>,
    /// Stores the internal `values` value.
    pub(crate) values: Vec<TokenStream2>,
    /// Stores the internal `docstrings` value.
    pub(crate) docstrings: Vec<TokenStream2>,
    /// Stores the internal `tables` value.
    pub(crate) tables: Vec<TokenStream2>,
}

/// Shared metadata for scenario test generation.
#[derive(Debug, Clone)]
pub(crate) struct ScenarioMetadata<'a> {
    /// Stores the internal `feature_path` value.
    pub(crate) feature_path: &'a FeaturePath,
    /// Stores the internal `scenario_name` value.
    pub(crate) scenario_name: &'a ScenarioName,
    /// Stores the internal `scenario_line` value.
    pub(crate) scenario_line: u32,
    /// Stores the internal `tags` value.
    pub(crate) tags: &'a [String],
    /// Stores the internal `block` value.
    pub(crate) block: &'a syn::Block,
    /// Stores the internal `allow_skipped` value.
    pub(crate) allow_skipped: bool,
    /// Whether to generate async step execution code.
    pub(crate) is_async: bool,
    /// Expected return kind for the scenario body.
    pub(crate) return_kind: ScenarioReturnKind,
    /// Optional harness adapter type path for execution delegation.
    pub(crate) harness: Option<&'a syn::Path>,
    /// Base harness API path selected once at the expansion boundary.
    pub(crate) harness_api_path: Option<TokenStream2>,
}

impl<'a> ScenarioMetadata<'a> {
    /// Provides the internal `literals_input` operation.
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
    /// Stores the internal `processed_steps` value.
    pub(crate) processed_steps: ProcessedSteps,
    /// Stores the internal `metadata` value.
    pub(crate) metadata: ScenarioMetadata<'a>,
}

/// Configuration for generating test tokens for scenario outlines.
#[derive(Debug)]
pub(crate) struct OutlineTestTokensConfig<'a> {
    /// Processed steps for each Examples row (one set per row).
    pub(crate) all_rows_steps: Vec<ProcessedStepTokens>,
    pub(crate) metadata: ScenarioMetadata<'a>,
}

#[derive(Debug)]
/// Internal data used by the macros implementation.
pub(super) struct ScenarioLiterals {
    /// Stores the internal `allow_literal` value.
    pub(super) allow_literal: syn::LitBool,
    /// Stores the internal `feature_literal` value.
    pub(super) feature_literal: syn::LitStr,
    /// Stores the internal `scenario_literal` value.
    pub(super) scenario_literal: syn::LitStr,
    /// Stores the internal `scenario_line_literal` value.
    pub(super) scenario_line_literal: syn::LitInt,
    /// Stores the internal `tag_literals` value.
    pub(super) tag_literals: Vec<syn::LitStr>,
}

#[derive(Debug, Clone, Copy)]
/// Internal data used by the macros implementation.
pub(crate) struct ScenarioLiteralsInput<'a> {
    /// Stores the internal `feature_path` value.
    pub(super) feature_path: &'a FeaturePath,
    /// Stores the internal `scenario_name` value.
    pub(super) scenario_name: &'a ScenarioName,
    /// Stores the internal `scenario_line` value.
    pub(super) scenario_line: u32,
    /// Stores the internal `tags` value.
    pub(super) tags: &'a [String],
    /// Stores the internal `allow_skipped` value.
    pub(super) allow_skipped: bool,
}

#[derive(Debug)]
/// Internal data used by the macros implementation.
pub(super) struct CodeComponents {
    /// Stores the internal `step_executor` value.
    pub(super) step_executor: TokenStream2,
    /// Stores the internal `skip_extractor` value.
    pub(super) skip_extractor: TokenStream2,
    /// Stores the internal `scenario_guard` value.
    pub(super) scenario_guard: TokenStream2,
    /// Stores the internal `step_executor_loop` value.
    pub(super) step_executor_loop: TokenStream2,
    /// Stores the internal `skip_handler` value.
    pub(super) skip_handler: TokenStream2,
}

#[derive(Debug, Clone, Copy)]
/// Internal data used by the macros implementation.
pub(super) struct TokenAssemblyContext<'a> {
    /// Stores the internal `ctx_prelude` value.
    pub(super) ctx_prelude: &'a [TokenStream2],
    /// Stores the internal `ctx_inserts` value.
    pub(super) ctx_inserts: &'a [TokenStream2],
    /// Stores the internal `ctx_postlude` value.
    pub(super) ctx_postlude: &'a [TokenStream2],
    /// Stores the internal `block` value.
    pub(super) block: &'a TokenStream2,
}

impl<'a> TokenAssemblyContext<'a> {
    /// Documents the internal `new` item.
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
