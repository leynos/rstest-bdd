//! Gherkin `.feature` file indexing support.

use std::borrow::Cow;
use std::ops::Range;
use std::path::{Path, PathBuf};

use gherkin::GherkinEnv;

use super::{
    FeatureFileIndex, FeatureIndexError, IndexedDocstring, IndexedScenarioOutline, IndexedStep,
    IndexedTable,
};

mod docstring;
mod outline;
mod table;

use docstring::find_docstring_span;
use outline::{
    ScenarioStepIndices, build_scenario_outline, extract_example_columns, is_scenario_outline,
};

/// Accumulates indexed steps and scenario outlines during feature indexing.
///
/// Groups the mutable output vectors together to reduce parameter count in
/// `process_scenarios` and `process_rule`.
struct IndexingAccumulators<'a> {
    steps: &'a mut Vec<IndexedStep>,
    scenario_outlines: &'a mut Vec<IndexedScenarioOutline>,
}

#[derive(Clone, Copy, Debug)]
struct FeatureSource<'a>(&'a str);

impl<'a> FeatureSource<'a> {
    fn new(source: &'a str) -> Self {
        Self(source)
    }

    fn as_str(&self) -> &'a str {
        self.0
    }

    fn get(&self, range: Range<usize>) -> Option<&'a str> {
        self.0.get(range)
    }

    fn len(&self) -> usize {
        self.0.len()
    }
}

impl AsRef<str> for FeatureSource<'_> {
    fn as_ref(&self) -> &str {
        self.0
    }
}

impl<'a> From<&'a str> for FeatureSource<'a> {
    fn from(source: &'a str) -> Self {
        Self::new(source)
    }
}

/// Parse and index a `.feature` file from disk.
///
/// The returned index uses byte offsets within the (normalised) feature text,
/// matching the behaviour of `gherkin` which appends a trailing newline when
/// missing.
///
/// # Errors
///
/// Returns an error when the feature file cannot be read or when it cannot be
/// parsed as valid Gherkin.
///
/// # Examples
///
/// ```rust,no_run
/// use rstest_bdd_server::indexing::{index_feature_file, FeatureIndexError};
///
/// # fn main() -> Result<(), FeatureIndexError> {
/// let path = std::env::temp_dir().join("rstest-bdd-index-demo.feature");
/// std::fs::write(
///     &path,
///     "Feature: demo\n  Scenario: s\n    Given a message\n",
/// )
/// .expect("feature file write should succeed");
///
/// let index = index_feature_file(&path)?;
/// assert_eq!(index.steps.len(), 1);
/// # std::fs::remove_file(path).ok();
/// # Ok(())
/// # }
/// ```
pub fn index_feature_file(path: &Path) -> Result<FeatureFileIndex, FeatureIndexError> {
    let mut text = std::fs::read_to_string(path)?;
    normalise_trailing_newline(&mut text);
    index_feature_text(path.to_path_buf(), FeatureSource::new(&text))
}

/// Parse and index a `.feature` file from source text.
///
/// This is primarily intended for language-server integrations that receive
/// the saved document contents from the client and want to avoid a race with
/// filesystem writes.
///
/// # Errors
///
/// Returns an error when the feature text cannot be parsed as valid Gherkin.
///
/// # Examples
///
/// ```rust,no_run
/// use std::path::PathBuf;
///
/// use rstest_bdd_server::indexing::{index_feature_source, FeatureIndexError};
///
/// # fn main() -> Result<(), FeatureIndexError> {
/// let feature = "Feature: demo\n  Scenario: s\n    Given a message\n";
/// let index = index_feature_source(PathBuf::from("demo.feature"), feature)?;
/// assert_eq!(index.steps.len(), 1);
/// # Ok(())
/// # }
/// ```
pub fn index_feature_source(
    path: PathBuf,
    source: &str,
) -> Result<FeatureFileIndex, FeatureIndexError> {
    let source = normalise_source_text(source);
    index_feature_text(path, FeatureSource::new(source.as_ref()))
}

fn index_feature_text(
    path: PathBuf,
    source: FeatureSource<'_>,
) -> Result<FeatureFileIndex, FeatureIndexError> {
    let feature = gherkin::Feature::parse(source.as_str(), GherkinEnv::default())?;

    let mut steps = Vec::new();
    let mut scenario_outlines = Vec::new();

    // Index feature-level background steps and track their indices
    let feature_background_start = steps.len();
    if let Some(background) = feature.background.as_ref() {
        steps.extend(index_steps_for_container(source, &background.steps)?);
    }
    let feature_background_end = steps.len();
    let feature_background_indices: Vec<usize> =
        (feature_background_start..feature_background_end).collect();

    let mut accumulators = IndexingAccumulators {
        steps: &mut steps,
        scenario_outlines: &mut scenario_outlines,
    };

    process_scenarios(
        source,
        &feature.scenarios,
        &mut accumulators,
        &feature_background_indices,
    )?;

    for rule in &feature.rules {
        process_rule(source, rule, &mut accumulators, &feature_background_indices)?;
    }

    let example_columns = extract_example_columns(source, &feature);

    Ok(FeatureFileIndex {
        path,
        source: source.as_str().to_owned(),
        steps,
        example_columns,
        scenario_outlines,
    })
}

/// Process a list of scenarios, indexing their steps and building scenario outlines.
fn process_scenarios(
    source: FeatureSource<'_>,
    scenarios: &[gherkin::Scenario],
    accumulators: &mut IndexingAccumulators<'_>,
    background_step_indices: &[usize],
) -> Result<(), FeatureIndexError> {
    for scenario in scenarios {
        let step_start_index = accumulators.steps.len();
        accumulators
            .steps
            .extend(index_steps_for_container(source, &scenario.steps)?);
        let step_end_index = accumulators.steps.len();

        if is_scenario_outline(scenario) {
            let indices = ScenarioStepIndices {
                start: step_start_index,
                end: step_end_index,
                background: background_step_indices.to_vec(),
            };
            let outline = build_scenario_outline(source, scenario, indices);
            accumulators.scenario_outlines.push(outline);
        }
    }
    Ok(())
}

/// Process a rule, indexing its background (if present) and scenarios.
fn process_rule(
    source: FeatureSource<'_>,
    rule: &gherkin::Rule,
    accumulators: &mut IndexingAccumulators<'_>,
    feature_background_indices: &[usize],
) -> Result<(), FeatureIndexError> {
    // Index rule-level background steps and track their indices
    let rule_background_start = accumulators.steps.len();
    if let Some(background) = rule.background.as_ref() {
        accumulators
            .steps
            .extend(index_steps_for_container(source, &background.steps)?);
    }
    let rule_background_end = accumulators.steps.len();

    // Combine feature-level and rule-level background indices
    let mut combined_background_indices = feature_background_indices.to_vec();
    combined_background_indices.extend(rule_background_start..rule_background_end);

    process_scenarios(
        source,
        &rule.scenarios,
        accumulators,
        &combined_background_indices,
    )
}

fn normalise_trailing_newline(text: &mut String) {
    if !text.ends_with('\n') {
        text.push('\n');
    }
}

fn normalise_source_text(source: &str) -> Cow<'_, str> {
    if source.ends_with('\n') {
        return Cow::Borrowed(source);
    }
    Cow::Owned(format!("{source}\n"))
}

fn index_steps_for_container(
    source: FeatureSource<'_>,
    steps: &[gherkin::Step],
) -> Result<Vec<IndexedStep>, FeatureIndexError> {
    let mut indexed = Vec::with_capacity(steps.len());
    for step in steps {
        let table = step.table.as_ref().map(|t| IndexedTable {
            rows: t.rows.clone(),
            span: t.span,
        });

        let docstring = match step.docstring.as_ref() {
            Some(value) => {
                let start_from = table.as_ref().map_or(step.span.end, |table| table.span.end);
                let span = find_docstring_span(source, start_from)
                    .ok_or(FeatureIndexError::DocstringSpanNotFound(step.span))?;
                Some(IndexedDocstring {
                    value: value.clone(),
                    span,
                })
            }
            None => None,
        };

        indexed.push(IndexedStep {
            keyword: step.keyword.clone(),
            step_type: step.ty,
            text: step.value.clone(),
            span: step.span,
            docstring,
            table,
        });
    }
    Ok(indexed)
}

#[cfg(test)]
mod tests;
