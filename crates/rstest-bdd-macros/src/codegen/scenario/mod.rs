//! Code generation for scenario tests.
//!
//! This module coordinates code generation for a BDD scenario or outline across
//! eight focused sub-modules:
//!
//! - [`adapters`] — resolves adapter APIs and fallback diagnostics once per expansion boundary.
//! - [`boundary`] — applies shared adapter decisions at the macro-expansion boundary.
//! - [`domain`] — shared types for steps, examples, and docstrings.
//! - [`emit`] — the metadata projection and item assembler shared by both scenario shapes.
//! - [`helpers`] — step-processing utilities and case-attribute generators.
//! - [`metadata`] — feature-path and scenario-name wrappers for generated code.
//! - [`runtime`] — async wrapper and harness-orchestrated runtime tokens.
//! - [`test_attrs`] — ADR-0008 attribute-policy resolution, translating harness and runtime-mode
//!   hints into the correct set of test attributes (`#[rstest::rstest]`, `#[tokio::test]`,
//!   `#[gpui::test]`).
//!
//! The crate-internal entry point is [`generate_scenario_code`], which resolves
//! adapter API paths once, projects the shared metadata, and then dispatches to
//! the regular or outline assembly path. [`emit_scenario_test`] wraps the
//! finished body in the item both paths share.

use std::borrow::Cow;

use proc_macro2::TokenStream as TokenStream2;

mod adapters;

mod boundary;

mod domain;

mod emit;

mod helpers;

mod metadata;

mod runtime;

mod test_attrs;

use adapters::resolve_scenario_adapters;
pub(crate) use domain::*;
use emit::{ScenarioEmit, emit_scenario_test, scenario_metadata};
pub(crate) use helpers::process_steps;
use helpers::{
    generate_case_attrs,
    generate_indexed_case_attrs,
    process_steps_substituted,
    row_has_values,
};
pub(crate) use metadata::{FeaturePath, ScenarioName};
use runtime::{
    ContextIterators,
    OutlineTestTokensConfig,
    ProcessedSteps,
    ScenarioMetadata,
    TestTokensConfig,
    generate_test_tokens,
};

pub(crate) use crate::macros::scenarios::ScenariosRuntimeMode as RuntimeMode;
use crate::{
    macros::scenarios::ScenariosTestAttributeHint as TestAttributeHint,
    parsing::placeholder::contains_placeholders,
};

/// Return kinds supported by scenario bodies.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ScenarioReturnKind {
    /// The scenario body returns no value.
    Unit,
    /// The scenario body returns `Result<(), E>`.
    ResultUnit,
}

impl ScenarioReturnKind {
    /// Whether generated code must propagate a scenario-body error.
    pub(crate) fn is_fallible(self) -> bool { matches!(self, Self::ResultUnit) }
}

/// Configuration for generating code for a single scenario test.
pub(crate) struct ScenarioConfig<'a> {
    /// Attributes on the annotated function.
    pub(crate) attrs: &'a [syn::Attribute],
    /// Visibility of the function.
    pub(crate) vis: &'a syn::Visibility,
    /// Signature of the function.
    pub(crate) sig: &'a syn::Signature,
    /// Function body.
    pub(crate) block: &'a syn::Block,
    /// Fully qualified feature file path.
    pub(crate) feature_path: FeaturePath,
    /// Name of the scenario.
    pub(crate) scenario_name: ScenarioName,
    /// Steps in the scenario.
    pub(crate) steps: Vec<crate::parsing::feature::ParsedStep>,
    /// Examples table for scenario outlines.
    pub(crate) examples: Option<crate::parsing::examples::ExampleTable>,
    /// Whether the scenario permits skipping without failing the suite.
    pub(crate) allow_skipped: bool,
    /// Line number where the scenario is declared in the feature file.
    pub(crate) line: u32,
    /// Tags inherited from the feature and scenario declarations.
    pub(crate) tags: &'a [String],
    /// Runtime mode for test execution (sync or async/Tokio).
    pub(crate) runtime: RuntimeMode,
    /// Runtime mode used when resolving generated test attributes.
    pub(crate) attribute_runtime: RuntimeMode,
    /// Return shape expected from the scenario body.
    pub(crate) return_kind: ScenarioReturnKind,
    /// Optional harness adapter type path for compile-time trait assertion.
    pub(crate) harness: Option<&'a syn::Path>,
    /// Optional attribute policy type path for compile-time trait assertion.
    pub(crate) attributes: Option<&'a syn::Path>,
    /// Boundary resolution; direct code-generation tests may use the pure local resolver.
    pub(crate) resolutions: Option<&'a crate::codegen::SharedAdapterResolutions>,
    /// Boundary tokens for `#[scenario]`; `scenarios!` emits around its module.
    pub(crate) fallback_diagnostics: Option<&'a TokenStream2>,
    /// Closed step-library scope emitted into the executor.
    pub(crate) scope: TokenStream2,
}

/// Configuration for context iterators in scenario code generation.
pub(crate) struct ContextConfig<P, I, Q> {
    /// Tokens emitted before the generated scenario body.
    pub(crate) prelude: P,
    /// Tokens inserted into the generated scenario body.
    pub(crate) inserts: I,
    /// Tokens emitted after the generated scenario body.
    pub(crate) postlude: Q,
}

/// Determine whether inherited scenario tags permit skipping the scenario.
pub(crate) fn scenario_allows_skip(tags: &[String]) -> bool {
    tags.iter().any(|tag| tag == "@allow_skipped")
}

/// Checks if any step in the scenario contains placeholder tokens.
fn steps_contain_placeholders(steps: &[crate::parsing::feature::ParsedStep]) -> bool {
    steps.iter().any(|step| {
        contains_placeholders(&step.text)
            || step
                .docstring
                .as_ref()
                .is_some_and(|d| contains_placeholders(d))
            || step.table.as_ref().is_some_and(|t| {
                t.iter()
                    .any(|row| row.iter().any(|cell| contains_placeholders(cell)))
            })
    })
}

/// The shape-specific materials for one generated scenario item.
struct ScenarioItem<'a> {
    /// The test signature, with any shape-specific parameters inserted.
    signature: Cow<'a, syn::Signature>,
    /// Attributes generated once per Examples row.
    case_attrs: Vec<TokenStream2>,
    /// The generated test body.
    body: TokenStream2,
}

/// Generate the runtime test for a single scenario.
///
/// Both scenario shapes share this assembly path: the harness rejection, the
/// adapter resolution, the metadata projection, and the final item emission
/// happen once here. Only step processing, case attributes, and the hidden
/// case-index parameter differ, and those live in the two `assemble_*`
/// functions.
pub(crate) fn generate_scenario_code(
    config: &ScenarioConfig<'_>,
    ctx_prelude: impl Iterator<Item = TokenStream2>,
    ctx_inserts: impl Iterator<Item = TokenStream2>,
    ctx_postlude: impl Iterator<Item = TokenStream2>,
) -> TokenStream2 {
    if let Some(rejection) = reject_async_harness(config) {
        return rejection;
    }

    let adapters = resolve_scenario_adapters(config);
    let harness_resolution = adapters.resolutions.harness.as_ref();
    let attributes_resolution = adapters.resolutions.attributes.as_ref();
    let metadata = scenario_metadata(
        config,
        harness_resolution.map(|resolution| resolution.api_path.clone()),
    );
    let ctx = ContextConfig {
        prelude: ctx_prelude,
        inserts: ctx_inserts,
        postlude: ctx_postlude,
    };

    let is_outline_with_placeholders =
        config.examples.is_some() && steps_contain_placeholders(&config.steps);
    let item = if is_outline_with_placeholders {
        match assemble_outline_scenario(config, metadata, ctx) {
            Ok(item) => item,
            Err(error) => return error,
        }
    } else {
        assemble_regular_scenario(config, metadata, ctx)
    };

    let ScenarioItem {
        mut signature,
        case_attrs,
        body,
    } = item;
    emit_scenario_test(
        config,
        &mut signature,
        ScenarioEmit {
            harness_resolution,
            attributes_resolution,
            case_attrs: &case_attrs,
            body,
        },
    )
}

/// Reject unsupported combinations of a harness and an async scenario.
fn reject_async_harness(config: &ScenarioConfig<'_>) -> Option<TokenStream2> {
    if config.harness.is_none() || !config.runtime.is_async() {
        return None;
    }

    let err = syn::Error::new(
        proc_macro2::Span::call_site(),
        concat!(
            "combining `harness` with `async fn` scenarios is not supported; use a synchronous ",
            "scenario function with `TokioHarness` instead (the harness provides the Tokio runtime ",
            "for step functions)",
        ),
    );
    Some(err.into_compile_error())
}

/// Assemble the body, signature, and case attributes for a regular scenario.
///
/// A regular scenario consumes its parsed steps directly. Its signature is
/// borrowed unchanged, because it gains no hidden parameters.
fn assemble_regular_scenario<'a, P, I, Q>(
    config: &'a ScenarioConfig<'_>,
    metadata: ScenarioMetadata<'a>,
    ctx: ContextConfig<P, I, Q>,
) -> ScenarioItem<'a>
where
    P: Iterator<Item = TokenStream2>,
    I: Iterator<Item = TokenStream2>,
    Q: Iterator<Item = TokenStream2>,
{
    let (keyword_tokens, values, docstrings, tables) = process_steps(&config.steps);
    debug_assert_eq!(keyword_tokens.len(), config.steps.len());
    let test_config = TestTokensConfig {
        processed_steps: ProcessedSteps {
            keyword_tokens,
            values,
            docstrings,
            tables,
        },
        metadata,
    };
    let case_attrs = config
        .examples
        .as_ref()
        .map_or_else(Vec::new, generate_case_attrs);
    let body = generate_test_tokens(
        &test_config,
        ContextIterators::new(ctx.prelude, ctx.inserts, ctx.postlude),
    );

    ScenarioItem {
        signature: Cow::Borrowed(config.sig),
        case_attrs,
        body,
    }
}

/// Substitute each Examples row's placeholders into the scenario's steps.
///
/// Rows without values are skipped rather than rejected: a blank Examples line
/// is how a table stays readable between groups, and Gherkin itself does not
/// give it meaning. The first failing row aborts the whole outline, because a
/// partially generated `#[rstest]` case list would bind later cases to the
/// wrong row.
fn substituted_outline_steps<'a>(
    config: &'a ScenarioConfig<'_>,
) -> Option<(
    &'a crate::parsing::examples::ExampleTable,
    Result<Vec<helpers::ProcessedStepTokens>, TokenStream2>,
)> {
    let examples = config.examples.as_ref()?;
    let headers = ExampleHeaders::new(examples.headers.clone());
    let substituted = examples
        .rows
        .iter()
        .filter(|row| row_has_values(row))
        .map(|row| {
            let row = ExampleRow::new(row.clone());
            process_steps_substituted(&config.steps, &headers, &row)
        })
        .collect();

    Some((examples, substituted))
}

/// Assemble the body, signature, and case attributes for a scenario outline.
///
/// An outline substitutes each Examples row into the steps and prepends the
/// hidden case-index parameter, so the generated test can select the row's
/// substituted step array at runtime.
fn assemble_outline_scenario<'a, P, I, Q>(
    config: &'a ScenarioConfig<'_>,
    metadata: ScenarioMetadata<'a>,
    ctx: ContextConfig<P, I, Q>,
) -> Result<ScenarioItem<'a>, TokenStream2>
where
    P: Iterator<Item = TokenStream2>,
    I: Iterator<Item = TokenStream2>,
    Q: Iterator<Item = TokenStream2>,
{
    let Some((examples, substituted)) = substituted_outline_steps(config) else {
        let error = syn::Error::new(
            proc_macro2::Span::call_site(),
            "Scenario outline examples missing",
        );
        return Err(error.into_compile_error());
    };
    let all_rows_steps = substituted?;

    let outline_config = OutlineTestTokensConfig {
        all_rows_steps,
        metadata,
    };
    let case_attrs = generate_indexed_case_attrs(examples);
    let body = generate_test_tokens(
        &outline_config,
        ContextIterators::new(ctx.prelude, ctx.inserts, ctx.postlude),
    );

    // The hidden index parameter must come first so each `#[case(...)]` can
    // carry the row index ahead of the row's own values.
    let mut signature: Cow<'a, syn::Signature> = Cow::Owned(config.sig.clone());
    let case_idx_param: syn::FnArg = syn::parse_quote! {
        #[case] __rstest_bdd_case_idx: usize
    };
    signature.to_mut().inputs.insert(0, case_idx_param);

    Ok(ScenarioItem {
        signature,
        case_attrs,
        body,
    })
}

#[cfg(test)]
mod tests;
