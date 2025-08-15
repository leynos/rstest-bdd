//! Feature file loading and scenario extraction.

use gherkin::{Feature, GherkinEnv, Step};
use std::path::{Path, PathBuf};

use crate::parsing::examples::ExampleTable;
use crate::utils::errors::error_to_tokens;
use crate::validation::examples::validate_examples_in_feature_text;

/// Step extracted from a scenario with optional arguments (data table and doc string).
#[derive(Debug, PartialEq)]
pub(crate) struct ParsedStep {
    pub keyword: rstest_bdd::StepKeyword,
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

/// Convert a Gherkin step to a `ParsedStep`.
fn map_step(step: &Step) -> ParsedStep {
    let keyword = match step.keyword.as_str() {
        "And" => rstest_bdd::StepKeyword::And,
        "But" => rstest_bdd::StepKeyword::But,
        _ => step.ty.into(),
    };
    let table = step.table.as_ref().map(|t| t.rows.clone());
    let docstring = step.docstring.clone();
    ParsedStep {
        keyword,
        text: step.value.clone(),
        docstring,
        table,
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
    match std::fs::metadata(&feature_path) {
        Ok(meta) if meta.is_file() => {}
        Ok(_) => {
            let msg = format!("feature path is not a file: {}", feature_path.display());
            let err = syn::Error::new(proc_macro2::Span::call_site(), msg);
            return Err(error_to_tokens(&err));
        }
        Err(_) => {
            let msg = format!("feature file not found: {}", feature_path.display());
            let err = syn::Error::new(proc_macro2::Span::call_site(), msg);
            return Err(error_to_tokens(&err));
        }
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
        steps.extend(bg.steps.iter().map(map_step));
    }
    steps.extend(scenario.steps.iter().map(map_step));

    let examples = crate::parsing::examples::extract_examples(scenario)?;

    Ok(ScenarioData {
        name: scenario_name,
        steps,
        examples,
    })
}

#[cfg(test)]
mod tests;
