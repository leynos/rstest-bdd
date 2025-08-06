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

fn map_step_type(ty: StepType) -> rstest_bdd::StepKeyword {
    match ty {
        StepType::Given => rstest_bdd::StepKeyword::Given,
        StepType::When => rstest_bdd::StepKeyword::When,
        StepType::Then => rstest_bdd::StepKeyword::Then,
    }
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

    let mut steps = Vec::new();
    if let Some(bg) = &feature.background {
        steps.extend(
            bg.steps
                .iter()
                .map(|s| (map_step_type(s.ty), s.value.clone())),
        );
    }
    steps.extend(
        scenario
            .steps
            .iter()
            .map(|s| (map_step_type(s.ty), s.value.clone())),
    );

    let examples = crate::parsing::examples::extract_examples(scenario)?;

    Ok(ScenarioData {
        name: scenario_name,
        steps,
        examples,
    })
}

#[cfg(test)]
mod tests {
    //! Tests for feature parsing utilities.
    use super::*;
    use gherkin::{Background, LineCol, Scenario, Span, Step, StepType};

    fn mk_step(ty: StepType, value: &str) -> Step {
        Step {
            keyword: match ty {
                StepType::Given => "Given",
                StepType::When => "When",
                StepType::Then => "Then",
            }
            .to_string(),
            ty,
            value: value.to_string(),
            docstring: None,
            table: None,
            span: Span { start: 0, end: 0 },
            position: LineCol { line: 0, col: 0 },
        }
    }

    #[test]
    fn prepends_background_steps() {
        let feature = Feature {
            keyword: "Feature".into(),
            name: "example".into(),
            description: None,
            background: Some(Background {
                keyword: "Background".into(),
                name: String::new(),
                description: None,
                steps: vec![mk_step(StepType::Given, "a background step")],
                span: Span { start: 0, end: 0 },
                position: LineCol { line: 0, col: 0 },
            }),
            scenarios: vec![Scenario {
                keyword: "Scenario".into(),
                name: "run".into(),
                description: None,
                steps: vec![
                    mk_step(StepType::When, "an action"),
                    mk_step(StepType::Then, "a result"),
                ],
                examples: Vec::new(),
                tags: Vec::new(),
                span: Span { start: 0, end: 0 },
                position: LineCol { line: 0, col: 0 },
            }],
            rules: Vec::new(),
            tags: Vec::new(),
            span: Span { start: 0, end: 0 },
            position: LineCol { line: 0, col: 0 },
            path: None,
        };

        let ScenarioData { steps, .. } = extract_scenario_steps(&feature, Some(0))
            .unwrap_or_else(|_| panic!("scenario extraction failed"));
        assert_eq!(
            steps,
            vec![
                (
                    rstest_bdd::StepKeyword::Given,
                    "a background step".to_string()
                ),
                (rstest_bdd::StepKeyword::When, "an action".to_string()),
                (rstest_bdd::StepKeyword::Then, "a result".to_string()),
            ]
        );
    }
}
