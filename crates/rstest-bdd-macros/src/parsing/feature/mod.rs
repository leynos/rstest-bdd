//! Feature file loading and scenario extraction.

use dashmap::DashMap;
use gherkin::{Feature, GherkinEnv, Step, StepType};
use std::{
    path::{Path, PathBuf},
    sync::LazyLock,
};

use crate::parsing::examples::ExampleTable;
use crate::utils::errors::error_to_tokens;
cfg_if::cfg_if! {
    if #[cfg(feature = "compile-time-validation")] {
        use crate::validation::examples::validate_examples_in_feature_text;
    }
}

/// Step extracted from a scenario with optional arguments (data table and doc string).
#[derive(Debug, Clone)]
pub(crate) struct ParsedStep {
    pub keyword: crate::StepKeyword,
    pub text: String,
    pub docstring: Option<String>,
    pub table: Option<Vec<Vec<String>>>,
    #[cfg(feature = "compile-time-validation")]
    #[cfg_attr(docsrs, doc(cfg(feature = "compile-time-validation")))]
    /// Approximate span for diagnostics.
    pub(crate) span: proc_macro2::Span,
}

// Equality intentionally ignores `span` as spans vary between compilations.
// Compare only semantic fields to keep tests stable; update if new fields are added.
impl PartialEq for ParsedStep {
    fn eq(&self, other: &Self) -> bool {
        self.keyword == other.keyword
            && self.text == other.text
            && self.docstring == other.docstring
            && self.table == other.table
    }
}

impl Eq for ParsedStep {}

/// Name, steps, and optional examples extracted from a Gherkin scenario.
pub(crate) struct ScenarioData {
    pub name: String,
    pub steps: Vec<ParsedStep>,
    pub(crate) examples: Option<ExampleTable>,
}

/// Cache parsed features to avoid repeated filesystem IO.
static FEATURE_CACHE: LazyLock<DashMap<PathBuf, Feature>> = LazyLock::new(DashMap::new);

/// Map a textual step keyword and `StepType` to a `StepKeyword`.
///
/// Conjunction keywords such as "And" and "But" inherit the semantic
/// meaning of the preceding step but remain distinct for later resolution.
/// Matching is case-insensitive to tolerate unusual source casing.
#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "retained for future compile-time validation paths"
    )
)]
pub(crate) fn parse_step_keyword(kw: &str, ty: StepType) -> crate::StepKeyword {
    let lower = kw.trim().to_ascii_lowercase();
    if lower == "and" {
        return crate::StepKeyword::And;
    }
    if lower == "but" {
        return crate::StepKeyword::But;
    }
    match ty {
        StepType::Given => crate::StepKeyword::Given,
        StepType::When => crate::StepKeyword::When,
        StepType::Then => crate::StepKeyword::Then,
    }
}

// Note: historic helpers for conjunction resolution lived here; the codegen
// path now resolves conjunctions locally to avoid feature-gated deps.
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
        #[expect(
            clippy::expect_used,
            reason = "gherkin::StepType is limited to Given/When/Then"
        )]
        let keyword =
            crate::StepKeyword::try_from(step).expect("valid step keyword from gherkin::Step");
        let table = step.table.as_ref().map(|t| t.rows.clone());
        let docstring = step.docstring.clone();
        Self {
            keyword,
            text: step.value.clone(),
            docstring,
            table,
            #[cfg(feature = "compile-time-validation")]
            span: proc_macro2::Span::call_site(),
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
/// Emits a compile-time error (as tokens) when the feature path does not exist
/// or is not a regular file.
///
/// On parse errors, attempts to surface validation diagnostics for Examples
/// tables where possible.
pub(crate) fn parse_and_load_feature(path: &Path) -> Result<Feature, proc_macro2::TokenStream> {
    let feature_path = std::env::var("CARGO_MANIFEST_DIR")
        .map_or_else(|_| PathBuf::from(path), |dir| PathBuf::from(dir).join(path));

    // Canonicalise for stable cache keys; missing files fall back to the joined path.
    let canonical = std::fs::canonicalize(&feature_path).ok();
    if let Some(feature) = canonical.as_ref().and_then(|p| FEATURE_CACHE.get(p)) {
        return Ok(feature.clone());
    }
    if let Some(feature) = FEATURE_CACHE.get(&feature_path) {
        return Ok(feature.clone());
    }

    if let Err(err) = validate_feature_file_exists(&feature_path) {
        return Err(error_to_tokens(&err));
    }

    let feature = Feature::parse_path(&feature_path, GherkinEnv::default()).map_err(|err| {
        #[cfg(feature = "compile-time-validation")]
        {
            if let Ok(text) = std::fs::read_to_string(&feature_path) {
                if let Err(validation_err) = validate_examples_in_feature_text(&text) {
                    return validation_err;
                }
            }
        }
        let msg = format!("failed to parse feature file: {err}");
        error_to_tokens(&syn::Error::new(proc_macro2::Span::call_site(), msg))
    })?;

    let key = canonical.unwrap_or_else(|| feature_path.clone());
    FEATURE_CACHE.insert(key.clone(), feature.clone());
    if key != feature_path {
        FEATURE_CACHE.insert(feature_path.clone(), feature.clone());
    }

    Ok(feature)
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

    let parse = |step: &Step| -> Result<ParsedStep, proc_macro2::TokenStream> {
        Ok(ParsedStep::from(step))
    };

    let mut steps = Vec::new();
    if let Some(bg) = &feature.background {
        steps.extend(bg.steps.iter().map(parse).collect::<Result<Vec<_>, _>>()?);
    }
    steps.extend(
        scenario
            .steps
            .iter()
            .map(parse)
            .collect::<Result<Vec<_>, _>>()?,
    );

    let examples = crate::parsing::examples::extract_examples(scenario)?;

    Ok(ScenarioData {
        name: scenario_name,
        steps,
        examples,
    })
}

#[cfg(test)]
mod tests;
