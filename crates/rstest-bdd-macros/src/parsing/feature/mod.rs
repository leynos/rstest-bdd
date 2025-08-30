//! Feature file loading and scenario extraction.

use gherkin::{Feature, GherkinEnv, Step, StepType};
use std::path::{Path, PathBuf};

use crate::parsing::examples::ExampleTable;
use crate::utils::errors::error_to_tokens;
use crate::validation::examples::validate_examples_in_feature_text;

/// Step extracted from a scenario with optional arguments (data table and doc string).
#[derive(Debug, PartialEq)]
pub(crate) struct ParsedStep {
    pub keyword: crate::StepKeyword,
    pub text: String,
    pub docstring: Option<String>,
    pub table: Option<Vec<Vec<String>>>,
}

/// Name, steps, and optional examples extracted from a Gherkin scenario.
pub(crate) struct ScenarioData {
    pub name: String,
    pub steps: Vec<ParsedStep>,
    pub(crate) examples: Option<ExampleTable>,
}

/// Map a textual step keyword and `StepType` to a `StepKeyword`.
///
/// Conjunction keywords such as "And" and "But" inherit the semantic
/// meaning of the preceding step but remain distinct for later resolution.
/// Matching is case-insensitive to tolerate unusual source casing.
pub(crate) fn parse_step_keyword(kw: &str, ty: StepType) -> crate::StepKeyword {
    match kw.trim() {
        s if s.eq_ignore_ascii_case("and") => crate::StepKeyword::And,
        s if s.eq_ignore_ascii_case("but") => crate::StepKeyword::But,
        _ =>
        {
            #[expect(unreachable_patterns, reason = "panic on future StepType variants")]
            match ty {
                StepType::Given => crate::StepKeyword::Given,
                StepType::When => crate::StepKeyword::When,
                StepType::Then => crate::StepKeyword::Then,
                _ => panic!("unsupported step type: {ty:?}"),
            }
        }
    }
}

/// Return `true` if the keyword is a connective such as "And" or "But".
pub(crate) fn is_conjunction_keyword(kw: crate::StepKeyword) -> bool {
    matches!(kw, crate::StepKeyword::And | crate::StepKeyword::But)
}

/// Replace "And"/"But" with the previous keyword, falling back to itself when
/// no previous step exists.
pub(crate) fn resolve_conjunction_keyword(
    prev: &mut Option<crate::StepKeyword>,
    kw: crate::StepKeyword,
) -> crate::StepKeyword {
    if is_conjunction_keyword(kw) {
        prev.unwrap_or(kw)
    } else {
        *prev = Some(kw);
        kw
    }
}

/// Convert a Gherkin step to a `ParsedStep`.
///
/// Uses the textual keyword when present to honour conjunctions
/// (And/But). Falls back to the typed step when not a conjunction.
impl From<&Step> for ParsedStep {
    fn from(step: &Step) -> Self {
        // The Gherkin parser exposes both a textual keyword (e.g. "And") and a
        // typed variant (Given/When/Then). We prioritise the textual value so
        // that conjunctions are preserved and can be used to improve
        // diagnostics. Trimming avoids surprises from trailing spaces in
        // .feature files.
        let keyword = parse_step_keyword(&step.keyword, step.ty);
        let table = step.table.as_ref().map(|t| t.rows.clone());
        let docstring = step.docstring.clone();
        Self {
            keyword,
            text: step.value.clone(),
            docstring,
            table,
        }
    }
}

/// Validate that the feature path exists and points to a file.
fn validate_feature_file_exists(feature_path: &Path) -> Result<(), syn::Error> {
    match std::fs::metadata(feature_path) {
        Ok(meta) if meta.is_file() => Ok(()),
        Ok(_) => {
            let msg = format!("feature path is not a file: {}", feature_path.display());
            Err(syn::Error::new(proc_macro2::Span::call_site(), msg))
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            let msg = format!("feature file not found: {}", feature_path.display());
            Err(syn::Error::new(proc_macro2::Span::call_site(), msg))
        }
        Err(e) => {
            let msg = format!(
                "failed to access feature file ({}): {}",
                feature_path.display(),
                e
            );
            Err(syn::Error::new(proc_macro2::Span::call_site(), msg))
        }
    }
}

/// Parse and load a feature file from the given path.
///
/// Emits a compile-time error (as tokens) when:
/// - `CARGO_MANIFEST_DIR` is not set (macro not running under Cargo),
/// - the feature path does not exist, or
/// - the feature path is not a regular file.
///
/// On parse errors, attempts to surface validation diagnostics for Examples
/// tables where possible.
pub(crate) fn parse_and_load_feature(path: &Path) -> Result<Feature, proc_macro2::TokenStream> {
    let Ok(manifest_dir) = std::env::var("CARGO_MANIFEST_DIR") else {
        let err = syn::Error::new(
            proc_macro2::Span::call_site(),
            "CARGO_MANIFEST_DIR is not set. This variable is normally provided by Cargo. Ensure the macro runs within a Cargo build context.",
        );
        return Err(error_to_tokens(&err));
    };
    let feature_path = PathBuf::from(manifest_dir).join(path);
    if let Err(err) = validate_feature_file_exists(&feature_path) {
        return Err(error_to_tokens(&err));
    }

    Feature::parse_path(&feature_path, GherkinEnv::default()).map_err(|err| {
        if let Ok(text) = std::fs::read_to_string(&feature_path) {
            if let Err(validation_err) = validate_examples_in_feature_text(&text) {
                return validation_err;
            }
        }
        let msg = format!("failed to parse feature file: {err}");
        error_to_tokens(&syn::Error::new(proc_macro2::Span::call_site(), msg))
    })
}

/// Extract the scenario data for the given feature and optional index.
pub(crate) fn extract_scenario_steps(
    feature: &Feature,
    index: Option<usize>,
) -> Result<ScenarioData, proc_macro2::TokenStream> {
    let Some(scenario) = feature.scenarios.get(index.unwrap_or(0)) else {
        let err = syn::Error::new(
            proc_macro2::Span::call_site(),
            "scenario index out of range",
        );
        return Err(error_to_tokens(&err));
    };

    let scenario_name = scenario.name.clone();

    let mut steps = Vec::new();
    if let Some(bg) = &feature.background {
        steps.extend(bg.steps.iter().map(ParsedStep::from));
    }
    steps.extend(scenario.steps.iter().map(ParsedStep::from));

    let examples = crate::parsing::examples::extract_examples(scenario)?;

    Ok(ScenarioData {
        name: scenario_name,
        steps,
        examples,
    })
}

#[cfg(test)]
mod tests;
