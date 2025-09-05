//! Compile-time step registration and validation.
//!
//! Steps are stored per crate and keyword, enabling fast lookups without
//! scanning unrelated definitions. In non-strict mode missing local
//! definitions emit warnings so cross-crate steps remain usable. This module
//! stores step definitions registered via `#[given]`, `#[when]`, and `#[then]`
//! attribute macros and provides validation utilities for the `#[scenario]`
//! macro. It ensures that every Gherkin step in a scenario has a corresponding
//! step definition. Missing steps yield a `compile_error!` during macro
//! expansion, preventing tests from compiling with incomplete behaviour
//! coverage.

use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};

use crate::StepKeyword;
use crate::parsing::feature::ParsedStep;
use proc_macro_error::{abort, emit_warning};
use rstest_bdd::{StepPattern, extract_placeholders};

type Registry = HashMap<Box<str>, CrateDefs>;

#[derive(Default, Clone)]
struct CrateDefs {
    by_kw: HashMap<StepKeyword, Vec<&'static StepPattern>>,
}

impl CrateDefs {
    fn patterns(&self, kw: StepKeyword) -> &[&'static StepPattern] {
        self.by_kw.get(&kw).map_or(&[], Vec::as_slice)
    }
    fn is_empty(&self) -> bool {
        self.by_kw.values().all(Vec::is_empty)
    }
}

/// Global registry of step definitions.
///
/// Patterns are leaked into static memory and stored for the process lifetime.
/// Registration occurs during macro expansion and test initialisation, so
/// total allocation is bounded by the step definitions registered in the
/// current compilation session. Entries are grouped by crate to enable
/// fast, crate-scoped lookups during validation.
static REGISTERED: LazyLock<Mutex<Registry>> = LazyLock::new(|| Mutex::new(HashMap::new()));
static CURRENT_CRATE_ID: LazyLock<Box<str>> =
    LazyLock::new(|| normalise_crate_id(&current_crate_id_raw()));

/// Leak and compile a step pattern before registering.
///
/// Patterns are stored in a global static registry for the life of the
/// process. Macros therefore require 'static lifetimes, satisfied by
/// leaking each boxed pattern into static memory. Registration happens
/// during macro expansion and test initialisation, so the leak is bounded
/// by the number of step definitions registered in the current compilation
/// session, including those registered by tests.
/// Patterns are leaked into static memory because macros require `'static` lifetimes.
/// Registration occurs during macro expansion so the total leak is bounded.
fn register_step_impl(keyword: StepKeyword, pattern: &syn::LitStr, crate_id: Box<str>) {
    let leaked: &'static str = Box::leak(pattern.value().into_boxed_str());
    let step_pattern: &'static StepPattern = Box::leak(Box::new(StepPattern::new(leaked)));
    if let Err(e) = step_pattern.compile() {
        abort!(
            pattern.span(),
            "rstest-bdd-macros: Invalid step pattern '{}' in #[step] macro: {}",
            leaked,
            e
        );
    }
    #[expect(
        clippy::expect_used,
        reason = "lock poisoning is unrecoverable; panic with clear message"
    )]
    let mut reg = REGISTERED.lock().expect("step registry poisoned");
    // crate_id is already normalised
    let defs = reg.entry(crate_id).or_default();
    defs.by_kw.entry(keyword).or_default().push(step_pattern);
}

/// Record a step definition so scenarios can validate against it.
///
/// Steps are registered for the current crate.
pub(crate) fn register_step(keyword: StepKeyword, pattern: &syn::LitStr) {
    let crate_id = current_crate_id();
    register_step_impl(keyword, pattern, crate_id);
}

#[cfg(test)]
pub(crate) fn register_step_for_crate(keyword: StepKeyword, pattern: &str, crate_id: &str) {
    register_step_impl(
        keyword,
        &syn::LitStr::new(pattern, proc_macro2::Span::call_site()),
        normalise_crate_id(crate_id),
    );
}

/// Ensure all parsed steps have matching definitions.
///
/// In strict mode, missing steps cause compilation to fail. In non-strict mode,
/// the function emits warnings but allows compilation to continue so scenarios
/// can reference steps from other crates. Ambiguous step definitions within
/// this crate always produce an error.
///
/// # Errors
/// Returns a `syn::Error` when `strict` is `true` and a step lacks a matching
/// definition or when any step matches more than one definition.
pub(crate) fn validate_steps_exist(steps: &[ParsedStep], strict: bool) -> Result<(), syn::Error> {
    #[expect(
        clippy::expect_used,
        reason = "lock poisoning is unrecoverable; panic with clear message"
    )]
    let reg = REGISTERED.lock().expect("step registry poisoned");
    let current = current_crate_id();
    #[expect(
        clippy::option_if_let_else,
        reason = "explicit branch clarifies missing-definition warning"
    )]
    let local_defs = if let Some(d) = reg.get(current.as_ref()) {
        d.clone()
    } else {
        #[cfg(not(test))]
        emit_warning!(
            proc_macro2::Span::call_site(),
            "step registry has no definitions for crate ID '{}'. This may indicate a registry issue.",
            current.as_ref()
        );
        CrateDefs::default()
    };
    drop(reg);

    if local_defs.is_empty() && !strict {
        return Ok(());
    }

    let missing = collect_missing_steps(&local_defs, steps)?;
    handle_validation_result(&missing, strict)
}

fn collect_missing_steps(
    defs: &CrateDefs,
    steps: &[ParsedStep],
) -> Result<Vec<(proc_macro2::Span, String)>, syn::Error> {
    // Resolve conjunctions (And/But) deterministically to the preceding
    // primary keyword while preserving span-aware diagnostics.
    let resolved = resolve_keywords(steps);
    debug_assert_eq!(resolved.len(), steps.len());
    let mut missing = Vec::new();
    for (step, keyword) in steps.iter().zip(resolved) {
        if let Some(msg) = has_matching_step_definition(defs, keyword, step)? {
            let span = {
                #[cfg(feature = "compile-time-validation")]
                {
                    step.span
                }
                #[cfg(not(feature = "compile-time-validation"))]
                {
                    proc_macro2::Span::call_site()
                }
            };
            missing.push((span, msg));
        }
    }
    Ok(missing)
}

fn handle_validation_result(
    missing: &[(proc_macro2::Span, String)],
    strict: bool,
) -> Result<(), syn::Error> {
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

fn create_strict_mode_error(missing: &[(proc_macro2::Span, String)]) -> Result<(), syn::Error> {
    let msg = match missing {
        [(span, only)] => {
            return Err(syn::Error::new(*span, only.clone()));
        }
        _ => missing
            .iter()
            .map(|(_, m)| format!("  - {m}"))
            .collect::<Vec<_>>()
            .join("\n"),
    };
    let span = missing
        .first()
        .map_or_else(proc_macro2::Span::call_site, |(s, _)| *s);
    Err(syn::Error::new(span, msg))
}

fn emit_non_strict_warnings(missing: &[(proc_macro2::Span, String)]) {
    for (span, msg) in missing {
        let loc = span.start();
        if loc.line == 0 && loc.column == 0 {
            emit_warning!(
                proc_macro2::Span::call_site(),
                "rstest-bdd[non-strict]: {}",
                msg;
                note = "location unavailable (synthetic or default span)"
            );
        } else {
            emit_warning!(*span, "rstest-bdd[non-strict]: {}", msg);
        }
    }
}

fn has_matching_step_definition(
    defs: &CrateDefs,
    resolved: StepKeyword,
    step: &ParsedStep,
) -> Result<Option<String>, syn::Error> {
    let text = step.text.as_str();
    let patterns = defs.patterns(resolved);
    let matches: Vec<&'static StepPattern> = patterns
        .iter()
        .copied()
        .filter(|p| extract_placeholders(p, text.into()).is_ok())
        .collect();
    match matches.len() {
        0 => Ok(Some(format_missing_step_error(resolved, step, defs))),
        1 => Ok(None),
        _ => Err(format_ambiguous_step_error(&matches, step)),
    }
}

fn format_missing_step_error(resolved: StepKeyword, step: &ParsedStep, defs: &CrateDefs) -> String {
    let patterns = defs.patterns(resolved);
    let available_defs: Vec<&str> = patterns.iter().map(|p| p.as_str()).collect();
    let possible_matches: Vec<&str> = patterns
        .iter()
        .filter(|p| p.regex().is_match(step.text.as_str()))
        .map(|p| p.as_str())
        .collect();
    build_missing_step_message(resolved, step, &available_defs, &possible_matches)
}

fn format_ambiguous_step_error(matches: &[&'static StepPattern], step: &ParsedStep) -> syn::Error {
    let patterns: Vec<&str> = matches.iter().map(|p| p.as_str()).collect();
    let msg = format!(
        "Ambiguous step definition for '{}'.\n{}",
        step.text,
        patterns
            .iter()
            // Do not indent bullet lines to make matching consistent.
            .map(|p| format!("- {p}"))
            .collect::<Vec<_>>()
            .join("\n")
    );
    let span = {
        #[cfg(feature = "compile-time-validation")]
        {
            step.span
        }
        #[cfg(not(feature = "compile-time-validation"))]
        {
            proc_macro2::Span::call_site()
        }
    };
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

fn current_crate_id_raw() -> String {
    let name = std::env::var("CARGO_CRATE_NAME")
        .or_else(|_| std::env::var("CARGO_PKG_NAME"))
        .unwrap_or_else(|_| "unknown".to_owned());
    let out_dir = std::env::var("OUT_DIR").unwrap_or_default();
    format!("{name}:{out_dir}")
}

fn normalise_crate_id(id: &str) -> Box<str> {
    // Canonicalise the `OUT_DIR` component so repeated builds do not create
    // duplicate registry entries for the same crate.
    let (name, path) = id.split_once(':').unwrap_or((id, ""));
    if path.is_empty() {
        return name.into();
    }
    let canonical = std::path::Path::new(path)
        .canonicalize()
        .unwrap_or_else(|_| path.into())
        .to_string_lossy()
        .into_owned();
    format!("{name}:{canonical}").into_boxed_str()
}

fn current_crate_id() -> Box<str> {
    CURRENT_CRATE_ID.clone()
}

/// Resolve textual conjunctions ("And"/"But") to the semantic keyword of the
/// preceding step.
///
/// Seeds the chain with the first primary keyword, defaulting to `Given` when
/// none is found.
/// Returns an iterator yielding one keyword per input step.
pub(crate) fn resolve_keywords(
    steps: &[ParsedStep],
) -> impl ExactSizeIterator<Item = crate::StepKeyword> + '_ {
    let mut prev = steps
        .iter()
        .find_map(|s| match s.keyword {
            crate::StepKeyword::And | crate::StepKeyword::But => None,
            other => Some(other),
        })
        .or(Some(crate::StepKeyword::Given));
    let resolved = steps.iter().map(move |s| s.keyword.resolve(&mut prev));
    debug_assert_eq!(resolved.len(), steps.len());
    resolved
}
#[cfg(test)]
mod tests;
