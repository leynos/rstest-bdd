//! Implements the `#[scenario]` macro, wiring Rust tests to Gherkin scenarios
//! and surfacing compile-time diagnostics for invalid configuration. Supports
//! mutually exclusive selectors that either bind by index or match the
//! case-sensitive scenario title, defaulting to the first scenario when no
//! selector is supplied.

mod args;
mod paths;

use proc_macro::TokenStream;
use proc_macro2::Span;
use std::path::PathBuf;

use crate::codegen::scenario::{ScenarioConfig, generate_scenario_code};
use crate::parsing::feature::{ScenarioData, extract_scenario_steps, parse_and_load_feature};
use crate::parsing::tags::TagExpression;
use crate::utils::fixtures::extract_function_fixtures;
use crate::validation::parameters::process_scenario_outline_examples;

use self::args::{ScenarioArgs, ScenarioSelector};
use self::paths::canonical_feature_path;

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
    let tag_filter = match tag_filter {
        Some(lit) => {
            let raw = lit.value();
            let parsed = TagExpression::parse(&raw).map_err(|err| {
                proc_macro::TokenStream::from(
                    syn::Error::new(lit.span(), err.to_string()).into_compile_error(),
                )
            })?;
            Some((parsed, lit.span(), raw))
        }
        None => None,
    };

    if feature.scenarios.is_empty() {
        let err = syn::Error::new(path_lit.span(), "feature contains no scenarios");
        return Err(proc_macro::TokenStream::from(err.into_compile_error()));
    }

    let candidate_indices: Vec<usize> = match &selector {
        Some(ScenarioSelector::Index { value, .. }) => vec![*value],
        Some(ScenarioSelector::Name { value, span }) => {
            let idx = find_scenario_by_name(&feature, value, *span)
                .map_err(|err| proc_macro::TokenStream::from(err.into_compile_error()))?;
            vec![idx]
        }
        None => (0..feature.scenarios.len()).collect(),
    };

    let mut selected: Option<ScenarioData> = None;
    for idx in candidate_indices {
        let mut data =
            extract_scenario_steps(&feature, Some(idx)).map_err(proc_macro::TokenStream::from)?;
        let matches = if let Some((expr, _, _)) = &tag_filter {
            data.filter_by_tags(expr)
        } else {
            true
        };

        if matches {
            selected = Some(data);
            break;
        }
    }

    let Some(scenario_data) = selected else {
        if let Some((_, span, raw)) = &tag_filter {
            let message = match &selector {
                Some(ScenarioSelector::Index { value, .. }) => {
                    format!("scenario at index {value} does not match tag expression `{raw}`")
                }
                Some(ScenarioSelector::Name { value, .. }) => {
                    format!("scenario named \"{value}\" does not match tag expression `{raw}`")
                }
                None => format!("no scenarios matched tag expression `{raw}`"),
            };
            let err = syn::Error::new(*span, message);
            return Err(proc_macro::TokenStream::from(err.into_compile_error()));
        }

        let err = syn::Error::new(path_lit.span(), "no matching scenario found");
        return Err(proc_macro::TokenStream::from(err.into_compile_error()));
    };

    let feature_path_str = canonical_feature_path(&path);
    let ScenarioData {
        name: scenario_name,
        steps,
        examples,
        ..
    } = scenario_data;

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
            feature_path: feature_path_str,
            scenario_name,
            steps,
            examples,
        },
        ctx_inserts.into_iter(),
    ))
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
