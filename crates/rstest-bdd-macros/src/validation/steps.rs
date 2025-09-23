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

use camino::{Utf8Path, Utf8PathBuf};
use cap_std::{ambient_authority, fs_utf8::Dir};

use crate::StepKeyword;
use crate::parsing::feature::ParsedStep;
use crate::pattern::MacroPattern;
#[cfg(not(test))]
use proc_macro_error::emit_warning;

type Registry = HashMap<Box<str>, CrateDefs>;

#[derive(Default, Clone)]
struct CrateDefs {
    by_kw: HashMap<StepKeyword, Vec<&'static MacroPattern>>,
}

impl CrateDefs {
    fn patterns(&self, kw: StepKeyword) -> &[&'static MacroPattern] {
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
fn register_step_inner(keyword: StepKeyword, pattern: &syn::LitStr, crate_id: impl AsRef<str>) {
    let leaked: &'static str = Box::leak(pattern.value().into_boxed_str());
    let stored: &'static MacroPattern = Box::leak(Box::new(MacroPattern::new(leaked)));
    let _ = stored.regex(pattern.span());
    #[expect(
        clippy::expect_used,
        reason = "lock poisoning is unrecoverable; panic with clear message"
    )]
    let mut reg = REGISTERED.lock().expect("step registry poisoned");
    let crate_id = normalise_crate_id(crate_id.as_ref());
    let defs = reg.entry(crate_id).or_default();
    defs.by_kw.entry(keyword).or_default().push(stored);
}

/// Record a step definition so scenarios can validate against it.
///
/// Steps are registered for the current crate.
pub(crate) fn register_step(keyword: StepKeyword, pattern: &syn::LitStr) {
    register_step_inner(keyword, pattern, current_crate_id());
}

#[cfg(test)]
pub(crate) fn register_step_for_crate(keyword: StepKeyword, literal: &str, crate_id: &str) {
    let lit = syn::LitStr::new(literal, proc_macro2::Span::call_site());
    register_step_inner(keyword, &lit, crate_id);
}

/// Return the diagnostic span for a step.
///
/// ```ignore
/// use rstest_bdd_macros::{StepKeyword};
/// use rstest_bdd_macros::parsing::feature::ParsedStep;
/// use rstest_bdd_macros::validation::steps::get_step_span;
/// let step = ParsedStep {
///     keyword: StepKeyword::Given,
///     text: "a step".into(),
///     docstring: None,
///     table: None,
///     #[cfg(feature = "compile-time-validation")]
///     span: proc_macro2::Span::call_site(),
/// };
/// let _ = get_step_span(&step);
/// ```
fn get_step_span(step: &ParsedStep) -> proc_macro2::Span {
    #[cfg(feature = "compile-time-validation")]
    {
        step.span
    }
    #[cfg(not(feature = "compile-time-validation"))]
    {
        proc_macro2::Span::call_site()
    }
}

/// Search patterns for matches against a step.
///
/// ```ignore
/// use crate::pattern::MacroPattern;
/// use rstest_bdd_macros::{StepKeyword};
/// use rstest_bdd_macros::parsing::feature::ParsedStep;
/// use rstest_bdd_macros::validation::steps::find_step_matches;
/// let pattern = MacroPattern::new("a step");
/// let _ = pattern.regex(proc_macro2::Span::call_site());
/// let step = ParsedStep {
///     keyword: StepKeyword::Given,
///     text: "a step".into(),
///     docstring: None,
///     table: None,
///     #[cfg(feature = "compile-time-validation")]
///     span: proc_macro2::Span::call_site(),
/// };
/// let _ = find_step_matches(&step, &[&pattern]);
/// ```
fn find_step_matches(
    step: &ParsedStep,
    patterns: &[&'static MacroPattern],
) -> Result<Option<&'static MacroPattern>, Vec<&'static MacroPattern>> {
    let mut matches = Vec::new();
    for &pat in patterns {
        if pat
            .captures(get_step_span(step), step.text.as_str())
            .is_some()
        {
            matches.push(pat);
        }
    }
    match matches.len() {
        0 => Ok(None),
        1 => Ok(matches.pop()),
        _ => Err(matches),
    }
}

/// Validate a single step against registered definitions.
///
/// ```ignore
/// use crate::pattern::MacroPattern;
/// use rstest_bdd_macros::{StepKeyword};
/// use rstest_bdd_macros::parsing::feature::ParsedStep;
/// use rstest_bdd_macros::validation::steps::{validate_single_step, CrateDefs};
/// let pattern = MacroPattern::new("a step");
/// let _ = pattern.regex(proc_macro2::Span::call_site());
/// let mut defs = CrateDefs::default();
/// defs.by_kw.entry(StepKeyword::Given).or_default().push(&pattern);
/// let step = ParsedStep {
///     keyword: StepKeyword::Given,
///     text: "a step".into(),
///     docstring: None,
///     table: None,
///     #[cfg(feature = "compile-time-validation")]
///     span: proc_macro2::Span::call_site(),
/// };
/// let _ = validate_single_step(&step, StepKeyword::Given, Some(&defs));
/// ```
fn validate_single_step(
    step: &ParsedStep,
    kw: StepKeyword,
    defs: Option<&CrateDefs>,
) -> Result<Option<(proc_macro2::Span, String)>, syn::Error> {
    let patterns = defs.map_or(&[][..], |d| d.patterns(kw));
    match find_step_matches(step, patterns) {
        Ok(Some(_)) => Ok(None),
        Ok(None) => {
            let span = get_step_span(step);
            let msg = defs.map_or_else(
                || format_missing_step_error(kw, step, &CrateDefs::default()),
                |d| format_missing_step_error(kw, step, d),
            );
            Ok(Some((span, msg)))
        }
        Err(matches) => Err(format_ambiguous_step_error(&matches, step)),
    }
}

/// Decision on whether to validate steps.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RegistryDecision {
    Continue,
    Skip,
    WarnAndSkip,
}

/// Check whether the registry holds definitions for the current crate.
///
/// Returns a [`RegistryDecision`] indicating whether validation should proceed.
///
/// ```ignore
/// use rstest_bdd_macros::validation::steps::{validate_registry_state, CrateDefs};
/// let _ = validate_registry_state(Some(&CrateDefs::default()), "crate", true);
/// ```
fn validate_registry_state(
    defs: Option<&CrateDefs>,
    #[cfg_attr(test, expect(unused_variables, reason = "crate ID unused in tests"))] crate_id: &str,
    strict: bool,
) -> RegistryDecision {
    match defs {
        Some(d) if d.is_empty() && !strict => RegistryDecision::Skip,
        Some(_) => RegistryDecision::Continue,
        None => {
            if strict {
                RegistryDecision::Continue
            } else {
                #[cfg(not(test))]
                emit_warning!(
                    proc_macro2::Span::call_site(),
                    "step registry has no definitions for crate ID '{}'. This may indicate a registry issue.",
                    crate_id
                );
                RegistryDecision::WarnAndSkip
            }
        }
    }
}

/// Validate each step and collect missing ones.
///
/// ```ignore
/// use crate::pattern::MacroPattern;
/// use rstest_bdd_macros::{StepKeyword};
/// use rstest_bdd_macros::parsing::feature::ParsedStep;
/// use rstest_bdd_macros::validation::steps::{validate_individual_steps, CrateDefs};
/// let pattern = MacroPattern::new("a step");
/// let _ = pattern.regex(proc_macro2::Span::call_site());
/// let mut defs = CrateDefs::default();
/// defs.by_kw.entry(StepKeyword::Given).or_default().push(&pattern);
/// let steps = [ParsedStep {
///     keyword: StepKeyword::Given,
///     text: "a step".into(),
///     docstring: None,
///     table: None,
///     #[cfg(feature = "compile-time-validation")]
///     span: proc_macro2::Span::call_site(),
/// }];
/// let _ = validate_individual_steps(&steps, Some(&defs));
/// ```
fn validate_individual_steps(
    steps: &[ParsedStep],
    defs: Option<&CrateDefs>,
) -> Result<Vec<(proc_macro2::Span, String)>, syn::Error> {
    steps
        .iter()
        .zip(resolve_keywords(steps))
        .map(|(step, kw)| validate_single_step(step, kw, defs))
        .collect::<Result<Vec<_>, _>>()
        .map(|res| res.into_iter().flatten().collect())
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
    let defs = reg.get(current.as_ref());
    match validate_registry_state(defs, current.as_ref(), strict) {
        RegistryDecision::Continue => {}
        RegistryDecision::Skip | RegistryDecision::WarnAndSkip => return Ok(()),
    }
    let missing = validate_individual_steps(steps, defs)?;
    drop(reg);
    handle_validation_result(&missing, strict)
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

#[cfg_attr(test, expect(unused_variables, reason = "test warnings"))]
fn emit_non_strict_warnings(missing: &[(proc_macro2::Span, String)]) {
    #[cfg(not(test))]
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

fn format_missing_step_error(resolved: StepKeyword, step: &ParsedStep, defs: &CrateDefs) -> String {
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

fn format_ambiguous_step_error(matches: &[&'static MacroPattern], step: &ParsedStep) -> syn::Error {
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
    let original = Utf8Path::new(path);
    let canonical = Dir::open_ambient_dir(".", ambient_authority())
        .and_then(|dir| dir.canonicalize(original))
        .map(|resolved| {
            if resolved.is_absolute() {
                resolved
            } else {
                std::env::current_dir()
                    .ok()
                    .and_then(|cwd| Utf8PathBuf::from_path_buf(cwd).ok())
                    .map(|cwd| cwd.join(&resolved))
                    .map_or_else(
                        || {
                            original
                                .canonicalize_utf8()
                                .unwrap_or_else(|_| original.to_owned())
                        },
                        |joined| joined.as_path().canonicalize_utf8().unwrap_or(joined),
                    )
            }
        })
        .or_else(|_| original.canonicalize_utf8())
        .unwrap_or_else(|_| original.to_owned());
    let canonical = canonical.into_string();
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
