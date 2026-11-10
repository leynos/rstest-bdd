//! Implements the `#[scenario]` macro, wiring Rust tests to Gherkin scenarios
//! and surfacing compile-time diagnostics for invalid configuration. Supports
//! mutually exclusive selectors that either bind by index or match the
//! case-sensitive scenario title, defaulting to the first scenario when no
//! selector is supplied. An optional `tags = "…"` argument filters candidate
//! scenarios before selector resolution, keeping tests focused on the relevant
//! examples.
//!
//! Tag expressions combine case-sensitive tag names with the operators `not`,
//! `and`, and `or`. The parser applies the precedence `not` > `and` > `or`, and
//! parentheses may override the default binding. For instance, the following
//! test only executes scenarios tagged `@fast` while excluding any marked as
//! `@wip` or `@flaky`:
//!
//! ```ignore
//! #[scenario(
//!     "tests/features/filtering.feature",
//!     tags = "@fast and not (@wip or @flaky)"
//! )]
//! fn fast_stable_cases() {}
//! ```

mod args;
pub(super) mod paths;
mod return_kind;
mod selection;

use std::path::PathBuf;

use proc_macro::TokenStream;
use proc_macro2::Span;

#[rustfmt::skip]
use crate::codegen::scenario::{
    generate_scenario_code, FeaturePath, RuntimeMode, ScenarioConfig, ScenarioName,
};
#[rustfmt::skip]
use crate::parsing::feature::{
    parse_and_load_feature, ScenarioData,
};
use self::{
    args::{ScenarioArgs, library_scope_tokens, library_validation_names},
    paths::manifest_relative_feature_path,
    return_kind::classify_scenario_return,
    selection::{ensure_feature_not_empty, resolve_candidate_indices, select_scenario},
};
use crate::{
    parsing::tags::TagExpression,
    utils::fixtures::{FixtureBindingCode, extract_function_fixtures},
    validation::{
        parameters::process_scenario_outline_examples,
        placeholder::{ExampleHeaders, validate_step_placeholders},
    },
};

/// Internal data used by the macros implementation.
struct ScenarioTagFilter {
    /// Stores the internal `expr` value.
    expr: TagExpression,
    /// Stores the internal `span` value.
    span: Span,
    /// Stores the internal `raw` value.
    raw: String,
}

/// Encapsulates the data needed to search and filter scenarios.
#[derive(Copy, Clone)]
struct ScenarioLookup<'a> {
    /// Stores the internal `feature` value.
    feature: &'a gherkin::Feature,
    /// Stores the internal `candidate_indices` value.
    candidate_indices: &'a [usize],
    /// Stores the internal `tag_filter` value.
    tag_filter: Option<&'a ScenarioTagFilter>,
}

/// Provides the internal `scenario` operation.
pub(crate) fn scenario(attr: TokenStream, item: TokenStream) -> TokenStream {
    let args = syn::parse_macro_input!(attr as ScenarioArgs);
    let item_fn = syn::parse_macro_input!(item as syn::ItemFn);
    match try_scenario(&args, item_fn) {
        Ok(tokens) => tokens,
        Err(err) => err,
    }
}

/// The scenario selected from the feature file, plus the config pieces derived
/// from the macro arguments rather than from the feature text.
struct ResolvedScenario {
    /// Reusable state that survives the selection phase.
    feature_path: String,
    /// The parsed scenario body.
    data: ScenarioData,
    /// How the generated test should execute.
    runtime: RuntimeMode,
    /// Closed step-library scope tokens, empty when the macro named none.
    scope: proc_macro2::TokenStream,
    /// Whether the scenario may be skipped without failing.
    allow_skipped: bool,
}

/// Locate the scenario the macro arguments select, then validate its steps.
///
/// This is the read-only half of expansion: it parses the feature, resolves the
/// selector and tag filter, and rejects a scenario whose placeholders or steps
/// do not compile. It mutates only the signature-adjacent arguments that later
/// expansion needs (`process_scenario_outline_examples`).
fn resolve_scenario(
    args: &ScenarioArgs,
    item_fn: &mut syn::ItemFn,
) -> std::result::Result<ResolvedScenario, TokenStream> {
    let ScenarioArgs {
        path: path_lit,
        selector,
        tag_filter,
        libraries,
        ..
    } = args;
    let scope = library_scope_tokens(libraries.as_deref());
    let path = PathBuf::from(path_lit.value());
    let sig = &mut item_fn.sig;

    // Retrieve cached feature to avoid repeated parsing.
    let feature = parse_and_load_feature(&path).map_err(proc_macro::TokenStream::from)?;
    let tag_filter = parse_tag_filter(tag_filter.clone())?;
    ensure_feature_not_empty(path_lit, &feature)?;
    let candidate_indices = resolve_candidate_indices(selector.as_ref(), &feature, path_lit)?;
    let data = select_scenario(
        ScenarioLookup {
            feature: &feature,
            candidate_indices: &candidate_indices,
            tag_filter: tag_filter.as_ref(),
        },
        selector.as_ref(),
        path_lit,
    )?;

    // Decision D3: the embedded feature path is manifest-relative within the
    // crate, absolute otherwise (see `manifest_relative_feature_path`).
    let feature_path = manifest_relative_feature_path(&path);
    let allow_skipped = crate::codegen::scenario::scenario_allows_skip(&data.tags);

    // Validate placeholder references in scenario outline steps
    if let Some(ref ex) = data.examples {
        validate_step_placeholders(&data.steps, ExampleHeaders::new(&ex.headers))
            .map_err(|e| proc_macro::TokenStream::from(e.into_compile_error()))?;
    }

    if let Some(err) = validate_steps_compile_time(&data.steps, libraries.as_deref()) {
        return Err(err);
    }

    process_scenario_outline_examples(sig, data.examples.as_ref())
        .map_err(proc_macro::TokenStream::from)?;

    let runtime = if sig.asyncness.is_some() {
        RuntimeMode::TokioCurrentThread
    } else {
        RuntimeMode::Sync
    };

    Ok(ResolvedScenario {
        feature_path,
        data,
        runtime,
        scope,
        allow_skipped,
    })
}

/// Wire the scenario's fixture parameters into the generated step context.
///
/// A fixture whose type is `Result<T, E>` or `StepResult<T, E>` is propagated
/// with `?`, so the scenario must return a fallible type for that generated
/// code to compile. Rejecting the mismatch here produces the diagnostic
/// against the user's signature instead of a `?`-operator error inside
/// generated tokens.
fn resolve_fixture_setup(
    sig: &mut syn::Signature,
    return_kind: crate::codegen::scenario::ScenarioReturnKind,
) -> std::result::Result<FixtureBindingCode, TokenStream> {
    let (_args, fixture_setup) = extract_function_fixtures(sig)
        .map_err(|err| proc_macro::TokenStream::from(err.into_compile_error()))?;

    if fixture_setup.has_result_fixtures && !return_kind.is_fallible() {
        let err = syn::Error::new_spanned(
            &sig.output,
            concat!(
                "scenarios with fallible fixtures (`Result<T, E>` or `StepResult<T, E>`) ",
                "must return `Result<(), E>` or `StepResult<(), E>` to propagate ",
                "fixture initialization errors",
            ),
        );
        return Err(proc_macro::TokenStream::from(err.into_compile_error()));
    }

    Ok(fixture_setup)
}

/// Provides the internal `try_scenario` operation.
fn try_scenario(
    args: &ScenarioArgs,
    mut item_fn: syn::ItemFn,
) -> std::result::Result<TokenStream, TokenStream> {
    let path_lit = &args.path;
    // Classify the return shape first so an unsupported signature is reported
    // against the user's `fn` rather than against the feature file it also
    // happens to name.
    let return_kind = classify_scenario_return(&item_fn.sig)
        .map_err(|err| proc_macro::TokenStream::from(err.into_compile_error()))?;

    let ResolvedScenario {
        feature_path,
        data: scenario_data,
        runtime,
        scope,
        allow_skipped,
    } = resolve_scenario(args, &mut item_fn)?;

    let sig = &mut item_fn.sig;
    let fixture_setup = resolve_fixture_setup(sig, return_kind)?;

    let path = PathBuf::from(path_lit.value());
    // `#[scenario]` owns this expansion boundary, so it emits each fallback
    // diagnostic once before its generated code consumes the shared decision.
    let resolutions = crate::codegen::SharedAdapterResolutions::resolve(
        args.harness.as_ref(),
        args.attributes.as_ref(),
    );
    let fallback_diagnostics = resolutions.emit_diagnostics();
    let ScenarioData {
        name: scenario_name,
        steps,
        examples,
        tags,
        line,
    } = scenario_data;

    let config = ScenarioConfig {
        attrs: &item_fn.attrs,
        vis: &item_fn.vis,
        sig,
        block: &item_fn.block,
        feature_path: FeaturePath::new(feature_path),
        scenario_name: ScenarioName::new(scenario_name),
        steps,
        examples,
        allow_skipped,
        line,
        tags: &tags,
        runtime,
        attribute_runtime: runtime,
        return_kind,
        harness: args.harness.as_ref(),
        attributes: args.attributes.as_ref(),
        resolutions: Some(&resolutions),
        fallback_diagnostics: Some(&fallback_diagnostics),
        scope,
    };

    let generated = generate_scenario_code(
        &config,
        fixture_setup.prelude.into_iter(),
        fixture_setup.ctx_inserts.into_iter(),
        fixture_setup.postlude.into_iter(),
    );

    // Emit the Cargo rebuild-dependency tracking item as a sibling of the
    // generated test function, at item scope (Decision D0 in the 10.3.3
    // ExecPlan): once per bound feature file, independent of scenario codegen,
    // so a `tags =` filter or a harness-replaced body cannot leave the file
    // untracked. Milestone 1 scaffold: emits nothing yet (see
    // `codegen::tracking`).
    let tracking = crate::codegen::tracking::feature_tracking_item(&path, path_lit.span());

    Ok(proc_macro::TokenStream::from(quote::quote! {
        #tracking
        #generated
    }))
}

/// Provides the internal `parse_tag_filter` operation.
fn parse_tag_filter(
    tag_filter: Option<syn::LitStr>,
) -> Result<Option<ScenarioTagFilter>, TokenStream> {
    tag_filter
        .map(|lit| {
            let raw = lit.value();
            TagExpression::parse(&raw)
                .map(|expr| ScenarioTagFilter {
                    expr,
                    span: lit.span(),
                    raw,
                })
                .map_err(|err| {
                    proc_macro::TokenStream::from(
                        syn::Error::new(lit.span(), err.to_string()).into_compile_error(),
                    )
                })
        })
        .transpose()
}

/// Validate steps at compile time, optionally against a closed library list.
///
/// Scoped and unscoped validation differ only in which validator runs; the
/// strictness flag is a build-time choice, and a build with neither validation
/// feature skips the check rather than failing it. Keeping both paths in one
/// function means the feature selection cannot drift between them.
fn validate_steps_compile_time(
    steps: &[crate::parsing::feature::ParsedStep],
    libraries: Option<&[syn::Path]>,
) -> Option<TokenStream> {
    let libraries = libraries.map(library_validation_names);
    let res: Result<(), syn::Error> = {
        cfg_if::cfg_if! {
            if #[cfg(feature = "strict-compile-time-validation")] {
                run_step_validator(steps, libraries.as_deref(), true)
            } else if #[cfg(feature = "compile-time-validation")] {
                run_step_validator(steps, libraries.as_deref(), false)
            } else {
                let _ = (steps, libraries);
                Ok(())
            }
        }
    };
    res.err()
        .map(|error| proc_macro::TokenStream::from(error.into_compile_error()))
}

/// Dispatch to the scoped or unscoped validator for this build.
///
/// Only compiled when a validation feature is enabled: the `validation::steps`
/// module is itself feature-gated, so referencing it unconditionally here would
/// break builds that opt out of compile-time validation.
#[cfg(any(
    feature = "compile-time-validation",
    feature = "strict-compile-time-validation"
))]
fn run_step_validator(
    steps: &[crate::parsing::feature::ParsedStep],
    libraries: Option<&[Box<str>]>,
    strict: bool,
) -> Result<(), syn::Error> {
    libraries.map_or_else(
        || crate::validation::steps::validate_steps_exist(steps, strict),
        |libraries| {
            crate::validation::steps::validate_steps_exist_in_scope(steps, libraries, strict)
        },
    )
}
