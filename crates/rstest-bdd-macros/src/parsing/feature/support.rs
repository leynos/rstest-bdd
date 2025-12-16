//! Test support builders for feature parsing tests.

use super::{ParsedStep, ScenarioData as ExtractedScenarioData, extract_scenario_steps};
use gherkin::{Background, Examples, LineCol, Scenario, Span, Step, StepType, Table};

// Intentionally expect `unreachable_patterns` today (we match all current variants).
// If `gherkin::StepType` adds variants, this expectation stops triggering and
// the build fails, prompting updates to `kw()` (and any `TryFrom<StepType> for StepKeyword`).
#[expect(
    unreachable_patterns,
    reason = "StepType currently only has three variants"
)]
fn kw(ty: StepType) -> String {
    match ty {
        StepType::Given => "Given",
        StepType::When => "When",
        StepType::Then => "Then",
        _ => unreachable!("kw() only supports Given, When, and Then"),
    }
    .to_string()
}

fn zero_span() -> Span {
    Span { start: 0, end: 0 }
}

fn zero_pos() -> LineCol {
    LineCol { line: 0, col: 0 }
}

/// Construct a `Table` from an iterable of rows (each row being an iterable of cells).
///
/// The returned table uses a zero `span` and `position`, as these builders are
/// used in tests where source locations are irrelevant.
fn table_from_rows<I, R, S>(rows: I) -> Table
where
    I: IntoIterator<Item = R>,
    R: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    Table {
        rows: rows
            .into_iter()
            .map(|r| r.into_iter().map(|s| s.as_ref().to_string()).collect())
            .collect(),
        span: zero_span(),
        position: zero_pos(),
    }
}

pub(super) struct StepBuilder {
    ty: StepType,
    value: String,
    docstring: Option<String>,
    table: Option<gherkin::Table>,
    keyword: Option<String>,
}

impl StepBuilder {
    pub(super) fn new(ty: StepType, value: &str) -> Self {
        Self {
            ty,
            value: value.to_string(),
            docstring: None,
            table: None,
            keyword: None,
        }
    }

    pub(super) fn with_keyword(mut self, kw: &str) -> Self {
        self.keyword = Some(kw.to_string());
        self
    }

    pub(super) fn with_docstring(mut self, doc: &str) -> Self {
        self.docstring = Some(doc.to_string());
        self
    }

    pub(super) fn with_table<I, R, S>(mut self, rows: I) -> Self
    where
        I: IntoIterator<Item = R>,
        R: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        self.table = Some(table_from_rows(rows));
        self
    }

    pub(super) fn build(self) -> Step {
        Step {
            keyword: self.keyword.unwrap_or_else(|| kw(self.ty)),
            ty: self.ty,
            value: self.value,
            docstring: self.docstring,
            table: self.table,
            span: zero_span(),
            position: zero_pos(),
        }
    }
}

/// Builder for constructing `Examples` blocks in test support.
///
/// Produces `Examples` with zero span/position, empty tags, and no name or
/// description, mirroring the minimal fixtures used throughout the parsing
/// tests.
pub(super) struct ExamplesBuilder {
    table: Option<Table>,
}

impl ExamplesBuilder {
    /// Start building an Examples block.
    pub(super) fn new() -> Self {
        Self { table: None }
    }

    /// Set the Examples table rows (including the header row).
    pub(super) fn with_table<I, R, S>(mut self, rows: I) -> Self
    where
        I: IntoIterator<Item = R>,
        R: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        self.table = Some(table_from_rows(rows));
        self
    }

    /// Build the final `Examples` block with keyword "Examples".
    ///
    /// The returned block has no name or description, empty tags, and a zero
    /// span/position.
    pub(super) fn build(self) -> Examples {
        Examples {
            keyword: "Examples".into(),
            name: None,
            description: None,
            table: self.table,
            tags: Vec::new(),
            span: zero_span(),
            position: zero_pos(),
        }
    }
}

pub(super) struct FeatureBuilder {
    name: String,
    background: Option<Vec<Step>>,
    scenarios: Vec<Scenario>,
}

/// Internal parameter object staging scenario data before conversion to `gherkin::Scenario`.
///
/// Used by `push_scenario` to reduce argument count; the final `Scenario` will
/// have `description: None`, empty tags, and a zero span/position.
struct ScenarioData {
    keyword: String,
    name: String,
    steps: Vec<Step>,
    examples: Vec<Examples>,
}

impl ScenarioData {
    fn scenario(name: &str, steps: Vec<Step>) -> Self {
        Self {
            keyword: "Scenario".to_string(),
            name: name.to_string(),
            steps,
            examples: Vec::new(),
        }
    }

    fn scenario_outline(name: &str, steps: Vec<Step>, examples: Examples) -> Self {
        Self {
            keyword: "Scenario Outline".to_string(),
            name: name.to_string(),
            steps,
            examples: vec![examples],
        }
    }
}

impl FeatureBuilder {
    pub(super) fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            background: None,
            scenarios: Vec::new(),
        }
    }

    /// Convert staged `ScenarioData` into a `gherkin::Scenario` and append it to the feature.
    ///
    /// Sets `description: None`, empty `tags`, and zero `span`/`position`.
    fn push_scenario(&mut self, data: ScenarioData) {
        self.scenarios.push(Scenario {
            keyword: data.keyword,
            name: data.name,
            description: None,
            steps: data.steps,
            examples: data.examples,
            tags: Vec::new(),
            span: zero_span(),
            position: zero_pos(),
        });
    }

    pub(super) fn with_background(mut self, steps: Vec<Step>) -> Self {
        self.background = Some(steps);
        self
    }

    pub(super) fn with_scenario(mut self, name: &str, steps: Vec<Step>) -> Self {
        self.push_scenario(ScenarioData::scenario(name, steps));
        self
    }

    /// Add a Scenario Outline with the given name, steps, and Examples block.
    ///
    /// The Examples block is wrapped in a vector; the final scenario has the
    /// keyword "Scenario Outline", no description, empty tags, and a zero
    /// span/position.
    pub(super) fn with_scenario_outline(
        mut self,
        name: &str,
        steps: Vec<Step>,
        examples: Examples,
    ) -> Self {
        self.push_scenario(ScenarioData::scenario_outline(name, steps, examples));
        self
    }

    pub(super) fn build(self) -> gherkin::Feature {
        gherkin::Feature {
            keyword: "Feature".into(),
            name: self.name,
            description: None,
            background: self.background.map(|steps| Background {
                keyword: "Background".into(),
                name: String::new(),
                description: None,
                steps,
                span: zero_span(),
                position: zero_pos(),
            }),
            scenarios: self.scenarios,
            rules: Vec::new(),
            tags: Vec::new(),
            span: zero_span(),
            position: zero_pos(),
            path: None,
        }
    }
}

/// Build a feature from the provided builder, extract the steps for the scenario
/// at `scenario_index`, and assert that they match `expected_steps`.
pub(super) fn assert_feature_extraction(
    feature_builder: FeatureBuilder,
    expected_steps: &[ParsedStep],
    scenario_index: Option<usize>,
) {
    let feature = feature_builder.build();
    let ExtractedScenarioData { steps, .. } = extract_scenario_steps(&feature, scenario_index)
        .unwrap_or_else(|e| {
            panic!("failed to extract scenario steps at index {scenario_index:?}: {e}")
        });
    assert_eq!(
        steps, expected_steps,
        "extracted steps did not match expectation",
    );
}

#[cfg(test)]
mod tests {
    //! Tests for feature parsing test support builders.

    use super::*;
    use gherkin::StepType;

    #[test]
    fn step_builder_overrides_keyword_and_collects_arguments() {
        let step = StepBuilder::new(StepType::When, "value")
            .with_keyword("Given")
            .with_docstring("doc")
            .with_table(vec![vec!["a", "b"]])
            .build();

        assert_eq!(step.keyword.trim(), "Given");
        assert_eq!(step.docstring.as_deref(), Some("doc"));
        let Some(table) = step.table.as_ref() else {
            panic!("expected table on built step");
        };
        assert_eq!(table.rows, vec![vec!["a".to_string(), "b".to_string()]],);
    }

    #[test]
    fn feature_builder_constructs_feature_with_background_and_scenarios() {
        let feature = FeatureBuilder::new("demo")
            .with_background(vec![StepBuilder::new(StepType::Given, "setup").build()])
            .with_scenario(
                "scenario",
                vec![StepBuilder::new(StepType::Then, "result").build()],
            )
            .with_scenario_outline(
                "outline",
                vec![StepBuilder::new(StepType::When, "action").build()],
                ExamplesBuilder::new()
                    .with_table(vec![vec!["x"], vec!["1"]])
                    .build(),
            )
            .build();

        assert_eq!(feature.name, "demo");
        assert!(feature.background.is_some());
        assert_eq!(feature.scenarios.len(), 2);
        let Some(scenario) = feature.scenarios.first() else {
            panic!("expected scenarios");
        };
        assert_eq!(scenario.steps.len(), 1);

        // Assert second scenario is an outline with examples.
        let Some(outline) = feature.scenarios.get(1) else {
            panic!("expected second scenario");
        };
        assert_eq!(outline.keyword, "Scenario Outline");
        assert_eq!(outline.examples.len(), 1);
        let Some(examples) = outline.examples.first() else {
            panic!("expected outline to have examples");
        };
        let Some(examples_table) = examples.table.as_ref() else {
            panic!("outline should have examples table");
        };
        assert_eq!(
            examples_table.rows,
            vec![vec!["x".to_string()], vec!["1".to_string()]]
        );
    }

    #[test]
    fn assert_feature_extraction_validates_expected_steps() {
        let expected = [ParsedStep {
            keyword: crate::StepKeyword::Given,
            text: "step".to_string(),
            docstring: None,
            table: None,
            #[cfg(feature = "compile-time-validation")]
            span: proc_macro2::Span::call_site(),
        }];

        assert_feature_extraction(
            FeatureBuilder::new("demo").with_scenario(
                "scenario",
                vec![StepBuilder::new(StepType::Given, "step").build()],
            ),
            &expected,
            Some(0),
        );
    }

    #[test]
    #[should_panic(expected = "failed to extract scenario steps")]
    fn assert_feature_extraction_panics_on_oob_index() {
        let expected_step = StepBuilder::new(StepType::Given, "x").build();

        assert_feature_extraction(
            FeatureBuilder::new("demo")
                .with_scenario("only", vec![StepBuilder::new(StepType::Given, "x").build()]),
            &[ParsedStep::from(&expected_step)],
            Some(99),
        );
    }
}
