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
            .map(|p| format!("- {p}"))
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
        fmt_keyword(resolved),
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

fn fmt_keyword(kw: StepKeyword) -> &'static str {
    match kw {
        StepKeyword::Given => "Given",
        StepKeyword::When => "When",
        StepKeyword::Then => "Then",
        StepKeyword::And => "And",
        StepKeyword::But => "But",
    }
}
