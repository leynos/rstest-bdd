//! Scenario selection for the `#[scenario]` macro.
//!
//! Resolves the `index`/`name` selectors and optional tag filter against a
//! parsed feature, producing the scenario to bind or a compile-time
//! diagnostic explaining why nothing matched.

use proc_macro::TokenStream;
use proc_macro2::Span;

use crate::parsing::feature::{ScenarioData, extract_scenario_steps};

use super::ScenarioLookup;
use super::args::ScenarioSelector;

pub(super) fn ensure_feature_not_empty(
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

pub(super) fn resolve_candidate_indices(
    selector: Option<&ScenarioSelector>,
    feature: &gherkin::Feature,
    _path_lit: &syn::LitStr,
) -> Result<Vec<usize>, TokenStream> {
    match selector {
        Some(ScenarioSelector::Index { value, span }) => {
            resolve_index_selector(*value, *span, feature)
        }
        Some(ScenarioSelector::Name { value, span }) => {
            let idx = find_scenario_by_name(feature, value, *span)
                .map_err(|err| proc_macro::TokenStream::from(err.into_compile_error()))?;
            Ok(vec![idx])
        }
        None => Ok((0..feature.scenarios.len()).collect()),
    }
}

/// Validate an explicit `index` selector against the feature's scenario count.
fn resolve_index_selector(
    value: usize,
    span: Span,
    feature: &gherkin::Feature,
) -> Result<Vec<usize>, TokenStream> {
    if value >= feature.scenarios.len() {
        let count = feature.scenarios.len();
        let message = format!("scenario index out of range: {value} (available: {count})");
        let err = syn::Error::new(span, message);
        Err(proc_macro::TokenStream::from(err.into_compile_error()))
    } else {
        Ok(vec![value])
    }
}

pub(super) fn select_scenario(
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
            .is_none_or(|filter| data.filter_by_tags(&filter.expr));
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
    // serialize each tag set separately so callers can still spot gaps without
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
        None => Err(scenario_not_found_error(feature, name, span)),
        Some((idx, scenario)) => {
            let rest: Vec<(usize, &gherkin::Scenario)> = matches.collect();
            if rest.is_empty() {
                Ok(idx)
            } else {
                let mut all = Vec::with_capacity(rest.len() + 1);
                all.push((idx, scenario));
                all.extend(rest);
                Err(ambiguous_scenario_error(name, &all, span))
            }
        }
    }
}

/// Build the diagnostic for a scenario name that matched nothing.
fn scenario_not_found_error(feature: &gherkin::Feature, name: &str, span: Span) -> syn::Error {
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
    syn::Error::new(span, message)
}

/// Build the diagnostic for a scenario name shared by several scenarios.
fn ambiguous_scenario_error(
    name: &str,
    matches: &[(usize, &gherkin::Scenario)],
    span: Span,
) -> syn::Error {
    let indexes = matches
        .iter()
        .map(|(index, _)| index.to_string())
        .collect::<Vec<_>>()
        .join(", ");
    let lines = matches
        .iter()
        .map(|(_, matched_scenario)| matched_scenario.position.line.to_string())
        .collect::<Vec<_>>()
        .join(", ");
    let message = format!(
        "found multiple scenarios named \"{name}\"; use the `index` selector to disambiguate (matching indexes: {indexes}; lines: {lines})",
    );
    syn::Error::new(span, message)
}

#[cfg(test)]
#[expect(
    clippy::expect_used,
    reason = "test code uses infallible expects for clarity"
)]
mod tests {
    //! Unit tests for scenario-name resolution and selection diagnostics.

    use gherkin::{Feature, GherkinEnv};
    use proc_macro2::Span;

    use super::{find_scenario_by_name, format_available_tags, scenario_not_found_error};

    const TWO_SCENARIOS: &str = "Feature: demo\n\
                                 \x20 Scenario: first\n\
                                 \x20   Given a step\n\
                                 \x20 Scenario: second\n\
                                 \x20   Given a step\n";

    const DUPLICATE_SCENARIOS: &str = "Feature: demo\n\
                                       \x20 Scenario: twin\n\
                                       \x20   Given a step\n\
                                       \x20 Scenario: twin\n\
                                       \x20   Given a step\n";

    fn parse_feature(source: &str) -> Feature {
        // The Whitaker suite only recognizes `#[test]` functions as test
        // context, so this helper panics via `match` rather than `expect`.
        match Feature::parse(source, GherkinEnv::default()) {
            Ok(feature) => feature,
            Err(err) => panic!("feature source should parse: {err}"),
        }
    }

    #[test]
    fn finds_a_uniquely_named_scenario() {
        let feature = parse_feature(TWO_SCENARIOS);
        let index = find_scenario_by_name(&feature, "second", Span::call_site())
            .expect("uniquely named scenario should resolve");
        assert_eq!(index, 1);
    }

    #[test]
    fn missing_name_diagnostic_lists_available_titles() {
        let feature = parse_feature(TWO_SCENARIOS);
        let Err(err) = find_scenario_by_name(&feature, "third", Span::call_site()) else {
            panic!("unknown scenario name should not resolve");
        };
        let message = err.to_string();
        assert!(
            message.contains("scenario named \"third\" not found"),
            "diagnostic should name the missing scenario: {message}"
        );
        assert!(
            message.contains("available titles: \"first\", \"second\""),
            "diagnostic should list the available titles: {message}"
        );
    }

    #[test]
    fn missing_name_diagnostic_notes_empty_features() {
        let feature = parse_feature("Feature: demo\n");
        let message = scenario_not_found_error(&feature, "any", Span::call_site()).to_string();
        assert!(
            message.contains("feature contains no scenarios"),
            "diagnostic should note the empty feature: {message}"
        );
    }

    #[test]
    fn duplicate_names_produce_an_ambiguity_diagnostic() {
        let feature = parse_feature(DUPLICATE_SCENARIOS);
        let Err(err) = find_scenario_by_name(&feature, "twin", Span::call_site()) else {
            panic!("duplicate scenario names should be rejected");
        };
        let message = err.to_string();
        assert!(
            message.contains("found multiple scenarios named \"twin\""),
            "diagnostic should report the ambiguity: {message}"
        );
        assert!(
            message.contains("matching indexes: 0, 1"),
            "diagnostic should list the matching indexes: {message}"
        );
    }

    #[test]
    fn available_tags_placeholder_covers_no_examined_scenarios() {
        assert_eq!(format_available_tags(&[]), "available tags: <none>");
    }

    #[test]
    fn available_tags_serialize_each_examined_set() {
        let tag_sets = vec![vec!["@fast".to_string(), "@ui".to_string()], Vec::new()];
        assert_eq!(
            format_available_tags(&tag_sets),
            "available tags: @fast, @ui; <none>"
        );
    }
}
