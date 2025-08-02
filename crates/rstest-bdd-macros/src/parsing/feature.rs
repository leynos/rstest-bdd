//! Feature file loading and scenario extraction.

use gherkin::{Feature, GherkinEnv, StepType};
use std::path::{Path, PathBuf};

use crate::parsing::examples::ExampleTable;
use crate::utils::errors::error_to_tokens;
use crate::validation::examples::validate_examples_in_feature_text;

/// Name, steps, and optional examples extracted from a Gherkin scenario.
pub(crate) struct ScenarioData {
    pub(crate) name: String,
    pub(crate) steps: Vec<(rstest_bdd::StepKeyword, String)>,
    pub(crate) examples: Option<ExampleTable>,
}

/// Parse and load a feature file from the given path.
pub(crate) fn parse_and_load_feature(path: &Path) -> Result<Feature, proc_macro::TokenStream> {
    let Ok(manifest_dir) = std::env::var("CARGO_MANIFEST_DIR") else {
        let err = syn::Error::new(
            proc_macro2::Span::call_site(),
            "CARGO_MANIFEST_DIR is not set. This variable is normally provided by Cargo. Ensure the macro runs within a Cargo build context.",
        );
        return Err(error_to_tokens(&err));
    };
    let feature_path = PathBuf::from(manifest_dir).join(path);
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
) -> Result<ScenarioData, proc_macro::TokenStream> {
    let Some(scenario) = feature.scenarios.get(index.unwrap_or(0)) else {
        let err = syn::Error::new(
            proc_macro2::Span::call_site(),
            "scenario index out of range",
        );
        return Err(error_to_tokens(&err));
    };

    let scenario_name = scenario.name.clone();
    let steps = scenario
        .steps
        .iter()
        .map(|s| {
            let keyword = match s.ty {
                StepType::Given => rstest_bdd::StepKeyword::Given,
                StepType::When => rstest_bdd::StepKeyword::When,
                StepType::Then => rstest_bdd::StepKeyword::Then,
            };
            (keyword, s.value.clone())
        })
        .collect();

    let examples = crate::parsing::examples::extract_examples(scenario)?;

    Ok(ScenarioData {
        name: scenario_name,
        steps,
        examples,
    })
}
