//! Error message construction for step validation.

use super::{CrateDefs, get_step_span};
use crate::StepKeyword;
use crate::parsing::feature::ParsedStep;
use crate::pattern::MacroPattern;

pub(super) fn format_missing_step_error(
    resolved: StepKeyword,
    step: &ParsedStep,
    defs: &CrateDefs,
) -> String {
    let patterns = defs.patterns(resolved);
    let span = get_step_span(step);
    let available_defs: Vec<&str> = patterns.iter().map(|p| p.as_str()).collect();
    let possible_matches: Vec<&str> = patterns
        .iter()
        .filter(|p| p.regex(span).is_match(step.text.as_str()))
        .map(|p| p.as_str())
        .collect();
    build_missing_step_message(resolved, step, &available_defs, &possible_matches)
}

pub(super) fn format_ambiguous_step_error(
    matches: &[&'static MacroPattern],
    step: &ParsedStep,
) -> syn::Error {
    let patterns: Vec<&str> = matches.iter().map(|p| p.as_str()).collect();
    let msg = format!(
        "Ambiguous step definition for '{}'.\n{}",
        step.text,
        patterns
            .iter()
            .map(|p| format!("  - {p}"))
            .collect::<Vec<_>>()
            .join("\n"),
    );
    let span = get_step_span(step);
    syn::Error::new(span, msg)
}

fn build_missing_step_message(
    resolved: StepKeyword,
    step: &ParsedStep,
    available_defs: &[&str],
    possible_matches: &[&str],
) -> String {
    let mut msg = format!(
        "No matching step definition found for '{} {}'",
        resolved.display_name(),
        step.text
    );
    msg.push_str(&format_item_list(
        available_defs,
        "Available step definitions for this keyword:\n",
        |s| *s,
    ));
    msg.push_str(&format_item_list(
        possible_matches,
        "Possible matches:\n",
        |s| *s,
    ));
    msg
}

fn format_item_list<T, F>(items: &[T], header: &str, fmt_item: F) -> String
where
    F: Fn(&T) -> &str,
{
    if items.is_empty() {
        return String::new();
    }

    let mut msg = String::new();
    msg.push('\n');
    msg.push_str(header);
    for item in items {
        msg.push_str("  - ");
        msg.push_str(fmt_item(item));
        msg.push('\n');
    }
    msg
}

#[cfg(test)]
mod tests {
    //! Tests for step validation message formatting.

    use super::*;
    use proc_macro2::Span;

    fn leak_pattern(text: &str) -> &'static MacroPattern {
        let leaked: &'static str = Box::leak(text.to_string().into_boxed_str());
        Box::leak(Box::new(MacroPattern::new(leaked)))
    }

    fn parsed_step(text: &str) -> ParsedStep {
        ParsedStep {
            keyword: StepKeyword::Given,
            text: text.to_string(),
            docstring: None,
            table: None,
            #[cfg(feature = "compile-time-validation")]
            span: Span::call_site(),
        }
    }

    #[test]
    fn missing_step_error_lists_available_definitions_and_matches() {
        let pattern_a = leak_pattern("I have {item}");
        let pattern_b = leak_pattern("I have apples");
        let mut defs = CrateDefs::default();
        defs.by_kw
            .entry(StepKeyword::Given)
            .or_default()
            .extend([pattern_a, pattern_b]);

        let msg =
            format_missing_step_error(StepKeyword::Given, &parsed_step("I have pears"), &defs);

        assert!(msg.contains("Available step definitions"));
        assert!(msg.contains("- I have {item}"));
        assert!(msg.contains("- I have apples"));
        assert!(msg.contains("Possible matches"));
    }

    #[test]
    fn missing_step_error_omits_sections_when_no_definitions_exist() {
        let defs = CrateDefs::default();
        let msg =
            format_missing_step_error(StepKeyword::When, &parsed_step("perform an action"), &defs);

        assert!(!msg.contains("Available step definitions"));
        assert!(!msg.contains("Possible matches"));
    }

    #[test]
    fn ambiguous_step_error_lists_all_matching_patterns() {
        let pattern_a = leak_pattern("first pattern");
        let pattern_b = leak_pattern("second pattern");

        let err = format_ambiguous_step_error(&[pattern_a, pattern_b], &parsed_step("ambiguous"));
        let msg = err.to_string();

        assert!(msg.contains("Ambiguous step definition"));
        assert!(msg.contains("- first pattern"));
        assert!(msg.contains("- second pattern"));
    }
}
