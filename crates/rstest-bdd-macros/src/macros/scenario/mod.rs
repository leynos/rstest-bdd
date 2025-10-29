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

use proc_macro::TokenStream;
use proc_macro2::Span;
use std::path::PathBuf;

use crate::codegen::scenario::{generate_scenario_code, FeaturePath, ScenarioConfig, ScenarioName};
use crate::parsing::feature::{extract_scenario_steps, parse_and_load_feature, ScenarioData};
use crate::parsing::tags::TagExpression;
use crate::utils::fixtures::extract_function_fixtures;
use crate::validation::parameters::process_scenario_outline_examples;

use self::args::{ScenarioArgs, ScenarioSelector};
use self::paths::canonical_feature_path;

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
    }: ScenarioArgs,
    mut item_fn: syn::ItemFn,
) -> std::result::Result<TokenStream, TokenStream> {
    let path_lit = path;
    let path = PathBuf::from(path_lit.value());
    let attrs = &item_fn.attrs;
    let vis = &item_fn.vis;
    let sig = &mut item_fn.sig;
    let block = &item_fn.block;

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
    } = scenario_data;
    let allow_skipped = crate::codegen::scenario::scenario_allows_skip(&tags);

    if let Some(err) = validate_steps_compile_time(&steps) {
        return Err(err);
    }

    process_scenario_outline_examples(sig, examples.as_ref())
        .map_err(proc_macro::TokenStream::from)?;

    let (_args, ctx_inserts) = extract_function_fixtures(sig)
        .map_err(|err| proc_macro::TokenStream::from(err.into_compile_error()))?;

    Ok(generate_scenario_code(
        ScenarioConfig {
            attrs,
            vis,
            sig,
            block,
            feature_path: FeaturePath::new(feature_path_str),
            scenario_name: ScenarioName::new(scenario_name),
            steps,
            examples,
            allow_skipped,
        },
        ctx_inserts.into_iter(),
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

fn ensure_feature_not_empty(
    path_lit: &syn::LitStr,
    feature: &gherkin::Feature,
) -> Result<(), TokenStream> {
    if feature.scenarios.is_empty() {
        let msg = format!("feature `{}` contains no scenarios", path_lit.value());
        let err = syn::Error::new(path_lit.span(), msg);
        Err(proc_macro::TokenStream::from(err.into_compile_error()))
    } else {
        Ok(())
    }
}

fn resolve_candidate_indices(
    selector: Option<&ScenarioSelector>,
    feature: &gherkin::Feature,
    path_lit: &syn::LitStr,
) -> Result<Vec<usize>, TokenStream> {
    match selector {
        Some(ScenarioSelector::Index { value, span }) => {
            let value = *value;
            if value >= feature.scenarios.len() {
                let count = feature.scenarios.len();
                let noun = if count == 1 { "scenario" } else { "scenarios" };
                let message = format!(
                    "scenario index {value} out of range; feature `{}` defines {count} {noun}",
                    path_lit.value()
                );
                let err = syn::Error::new(*span, message);
                Err(proc_macro::TokenStream::from(err.into_compile_error()))
            } else {
                Ok(vec![value])
            }
        }
        Some(ScenarioSelector::Name { value, span }) => {
            let idx = find_scenario_by_name(feature, value, *span)
                .map_err(|err| proc_macro::TokenStream::from(err.into_compile_error()))?;
            Ok(vec![idx])
        }
        None => Ok((0..feature.scenarios.len()).collect()),
    }
}

fn select_scenario(
    lookup: ScenarioLookup<'_>,
    selector: Option<&ScenarioSelector>,
    path_lit: &syn::LitStr,
) -> Result<ScenarioData, TokenStream> {
    let mut examined_tag_sets: Vec<Vec<String>> = Vec::new();

    for &idx in lookup.candidate_indices {
        let mut data = extract_scenario_steps(lookup.feature, Some(idx))
            .map_err(proc_macro::TokenStream::from)?;
        let matches = lookup
            .tag_filter
            .map_or(true, |filter| data.filter_by_tags(&filter.expr));
        if matches {
            return Ok(data);
        }
        if lookup.tag_filter.is_some() {
            // Preserve the feature + scenario tags that callers evaluated so the
            // eventual diagnostic can explain which annotations were available.
            examined_tag_sets.push(data.tags.clone());
        }
    }

    lookup.tag_filter.map_or_else(
        || {
            debug_assert!(
                lookup.candidate_indices.is_empty(),
                "expected default scenario selection to succeed when no tag filter is provided",
            );
            // This branch should be unreachable in practice because
            // `ensure_feature_not_empty` guards empty features. Emit a specific
            // diagnostic if it ever triggers so consumers know the macro input
            // lacked scenarios.
            let message = format!("feature `{}` contains no scenarios", path_lit.value());
            let err = syn::Error::new(path_lit.span(), message);
            Err(proc_macro::TokenStream::from(err.into_compile_error()))
        },
        |filter| {
            let available_clause = format_available_tags(&examined_tag_sets);
            let message = match selector {
                Some(ScenarioSelector::Index { value, .. }) => {
                    format!(
                        "scenario at index {} does not match tag expression `{}`; {}",
                        value, filter.raw, available_clause
                    )
                }
                Some(ScenarioSelector::Name { value, .. }) => {
                    format!(
                        "scenario named \"{value}\" does not match tag expression `{}`; {}",
                        filter.raw, available_clause
                    )
                }
                None => format!(
                    "no scenarios matched tag expression `{}`; {}",
                    filter.raw, available_clause
                ),
            };
            let err = syn::Error::new(filter.span, message);
            Err(proc_macro::TokenStream::from(err.into_compile_error()))
        },
    )
}

fn format_available_tags(tag_sets: &[Vec<String>]) -> String {
    // Multiple scenarios can feed the diagnostic when no selector is supplied;
    // serialise each tag set separately so callers can still spot gaps without
    // losing the original grouping.
    if tag_sets.is_empty() {
        return "available tags: <none>".to_string();
    }

    let formatted_sets: Vec<String> = tag_sets
        .iter()
        .map(|tags| {
            if tags.is_empty() {
                "<none>".to_string()
            } else {
                tags.join(", ")
            }
        })
        .collect();

    format!("available tags: {}", formatted_sets.join("; "))
}

fn find_scenario_by_name(
    feature: &gherkin::Feature,
    name: &str,
    span: Span,
) -> Result<usize, syn::Error> {
    let mut matches = feature
        .scenarios
        .iter()
        .enumerate()
        .filter(|(_, scenario)| scenario.name == name);

    match matches.next() {
        None => {
            let available: Vec<String> = feature
                .scenarios
                .iter()
                .map(|scenario| format!("\"{}\"", scenario.name))
                .collect();
            let message = if available.is_empty() {
                format!("scenario named \"{name}\" not found; feature contains no scenarios")
            } else {
                let options = available.join(", ");
                format!("scenario named \"{name}\" not found; available titles: {options}")
            };
            Err(syn::Error::new(span, message))
        }
        Some((idx, scenario)) => {
            let rest: Vec<(usize, &gherkin::Scenario)> = matches.collect();
            if rest.is_empty() {
                Ok(idx)
            } else {
                let mut all = Vec::with_capacity(rest.len() + 1);
                all.push((idx, scenario));
                all.extend(rest);
                let indexes = all
                    .iter()
                    .map(|(index, _)| index.to_string())
                    .collect::<Vec<_>>()
                    .join(", ");
                let lines = all
                    .iter()
                    .map(|(_, matched_scenario)| matched_scenario.position.line.to_string())
                    .collect::<Vec<_>>()
                    .join(", ");
                let message = format!(
                    "found multiple scenarios named \"{name}\"; use the `index` selector to disambiguate (matching indexes: {indexes}; lines: {lines})",
                );
                Err(syn::Error::new(span, message))
            }
        }
    }
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
