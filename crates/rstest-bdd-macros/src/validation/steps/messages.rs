//! Formatting helpers for validation diagnostics.

use super::get_step_span;
use super::{CrateDefs, ParsedStep, StepKeyword};
use proc_macro2::Span;
use rstest_bdd::StepPattern;
use syn::Error;

#[cfg(not(test))]
use proc_macro_error::emit_warning;

pub(super) fn handle_validation_result(
    missing: &[(Span, String)],
    strict: bool,
) -> Result<(), Error> {
    if missing.is_empty() {
        return Ok(());
    }

    if strict {
        create_strict_mode_error(missing)
    } else {
        emit_non_strict_warnings(missing);
        Ok(())
    }
}

fn create_strict_mode_error(missing: &[(Span, String)]) -> Result<(), Error> {
    let msg = match missing {
        [(span, only)] => {
            return Err(Error::new(*span, only.clone()));
        }
        _ => missing
            .iter()
            .map(|(_, m)| format!("  - {m}"))
            .collect::<Vec<_>>()
            .join(
                "
",
            ),
    };
    let span = missing.first().map_or_else(Span::call_site, |(s, _)| *s);
    Err(Error::new(span, msg))
}

#[cfg_attr(test, expect(unused_variables, reason = "test warnings"))]
fn emit_non_strict_warnings(missing: &[(Span, String)]) {
    #[cfg(not(test))]
    for (span, msg) in missing {
        let loc = span.start();
        if loc.line == 0 && loc.column == 0 {
            emit_warning!(
                Span::call_site(),
                "rstest-bdd[non-strict]: {}",
                msg;
                note = "location unavailable (synthetic or default span)"
            );
        } else {
            emit_warning!(*span, "rstest-bdd[non-strict]: {}", msg);
        }
    }
}

pub(super) fn format_missing_step_error(
    resolved: StepKeyword,
    step: &ParsedStep,
    defs: &CrateDefs,
) -> String {
    let patterns = defs.patterns(resolved);
    let available_defs: Vec<&str> = patterns.iter().map(|p| p.as_str()).collect();
    let possible_matches: Vec<&str> = patterns
        .iter()
        .filter(|p| p.regex().is_match(step.text.as_str()))
        .map(|p| p.as_str())
        .collect();
    build_missing_step_message(resolved, step, &available_defs, &possible_matches)
}

pub(super) fn format_ambiguous_step_error(
    matches: &[&'static StepPattern],
    step: &ParsedStep,
) -> Error {
    let patterns: Vec<&str> = matches.iter().map(|p| p.as_str()).collect();
    let msg = format!(
        "Ambiguous step definition for '{}'.
{}",
        step.text,
        patterns
            .iter()
            // Do not indent bullet lines to make matching consistent.
            .map(|p| format!("- {p}"))
            .collect::<Vec<_>>()
            .join(
                "
"
            )
    );
    let span = get_step_span(step);
    Error::new(span, msg)
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
        "Available step definitions for this keyword:
",
        |s| *s,
    ));
    msg.push_str(&format_item_list(
        possible_matches,
        "Possible matches:
",
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
