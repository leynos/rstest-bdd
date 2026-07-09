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
mod paths;
mod return_kind;
mod selection;

use proc_macro::TokenStream;
use proc_macro2::Span;
use std::path::PathBuf;

#[rustfmt::skip]
use crate::codegen::scenario::{
    generate_scenario_code, FeaturePath, RuntimeMode, ScenarioConfig, ScenarioName,
};
#[rustfmt::skip]
use crate::parsing::feature::{
    parse_and_load_feature, ScenarioData,
};
use crate::parsing::tags::TagExpression;
use crate::utils::fixtures::extract_function_fixtures;
use crate::validation::parameters::process_scenario_outline_examples;
use crate::validation::placeholder::{ExampleHeaders, validate_step_placeholders};

use self::args::ScenarioArgs;
use self::paths::canonical_feature_path;
use self::return_kind::classify_scenario_return;
use self::selection::{ensure_feature_not_empty, resolve_candidate_indices, select_scenario};

struct ScenarioTagFilter {
    expr: TagExpression,
    span: Span,
    raw: String,
}

/// Encapsulates the data needed to search and filter scenarios.
#[derive(Copy, Clone)]
struct ScenarioLookup<'a> {
    feature: &'a gherkin::Feature,
    candidate_indices: &'a [usize],
    tag_filter: Option<&'a ScenarioTagFilter>,
}

pub(crate) fn scenario(attr: TokenStream, item: TokenStream) -> TokenStream {
    let args = syn::parse_macro_input!(attr as ScenarioArgs);
    let item_fn = syn::parse_macro_input!(item as syn::ItemFn);
    match try_scenario(args, item_fn) {
        Ok(tokens) => tokens,
        Err(err) => err,
    }
}

fn try_scenario(
    ScenarioArgs {
        path,
        selector,
        tag_filter,
        harness,
        attributes,
    }: ScenarioArgs,
    mut item_fn: syn::ItemFn,
) -> std::result::Result<TokenStream, TokenStream> {
    let path_lit = path;
    let path = PathBuf::from(path_lit.value());
    let attrs = &item_fn.attrs;
    let vis = &item_fn.vis;
    let sig = &mut item_fn.sig;
    let block = &item_fn.block;
    let return_kind = classify_scenario_return(sig)
        .map_err(|err| proc_macro::TokenStream::from(err.into_compile_error()))?;

    // Detect async function signature for runtime mode selection.
    let runtime = if sig.asyncness.is_some() {
        RuntimeMode::TokioCurrentThread
    } else {
        RuntimeMode::Sync
    };

    // Retrieve cached feature to avoid repeated parsing.
    let feature = parse_and_load_feature(&path).map_err(proc_macro::TokenStream::from)?;
    let tag_filter = parse_tag_filter(tag_filter)?;
    ensure_feature_not_empty(&path_lit, &feature)?;
    let candidate_indices = resolve_candidate_indices(selector.as_ref(), &feature, &path_lit)?;
    let scenario_data = select_scenario(
        ScenarioLookup {
            feature: &feature,
            candidate_indices: &candidate_indices,
            tag_filter: tag_filter.as_ref(),
        },
        selector.as_ref(),
        &path_lit,
    )?;

    let feature_path_str = canonical_feature_path(&path);
    let ScenarioData {
        name: scenario_name,
        steps,
        examples,
        tags,
        line,
    } = scenario_data;
    let allow_skipped = crate::codegen::scenario::scenario_allows_skip(&tags);

    // Validate placeholder references in scenario outline steps
    if let Some(ref ex) = examples {
        validate_step_placeholders(&steps, ExampleHeaders::new(&ex.headers))
            .map_err(|e| proc_macro::TokenStream::from(e.into_compile_error()))?;
    }

    if let Some(err) = validate_steps_compile_time(&steps) {
        return Err(err);
    }

    process_scenario_outline_examples(sig, examples.as_ref())
        .map_err(proc_macro::TokenStream::from)?;

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

    let ctx_prelude = fixture_setup.prelude;
    let ctx_inserts = fixture_setup.ctx_inserts;
    let ctx_postlude = fixture_setup.postlude;

    let config = ScenarioConfig {
        attrs,
        vis,
        sig,
        block,
        feature_path: FeaturePath::new(feature_path_str),
        scenario_name: ScenarioName::new(scenario_name),
        steps,
        examples,
        allow_skipped,
        line,
        tags: &tags,
        runtime,
        attribute_runtime: runtime,
        return_kind,
        harness: harness.as_ref(),
        attributes: attributes.as_ref(),
    };

    Ok(generate_scenario_code(
        &config,
        ctx_prelude.into_iter(),
        ctx_inserts.into_iter(),
        ctx_postlude.into_iter(),
    ))
}

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

fn validate_steps_compile_time(
    steps: &[crate::parsing::feature::ParsedStep],
) -> Option<TokenStream> {
    let res: Result<(), syn::Error> = {
        cfg_if::cfg_if! {
            if #[cfg(feature = "strict-compile-time-validation")] {
                crate::validation::steps::validate_steps_exist(steps, true)
            } else if #[cfg(feature = "compile-time-validation")] {
                crate::validation::steps::validate_steps_exist(steps, false)
            } else {
                let _ = steps;
                Ok(())
            }
        }
    };
    res.err()
        .map(|e| proc_macro::TokenStream::from(e.into_compile_error()))
}
