//! Test support builders for feature parsing tests.

use super::{extract_scenario_steps, ParsedStep, ScenarioData};
use gherkin::{Background, LineCol, Scenario, Span, Step, StepType};

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
        self.table = Some(gherkin::Table {
            rows: rows
                .into_iter()
                .map(|r| r.into_iter().map(|s| s.as_ref().to_string()).collect())
                .collect(),
            span: zero_span(),
            position: zero_pos(),
        });
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

pub(super) struct FeatureBuilder {
    name: String,
    background: Option<Vec<Step>>,
    scenarios: Vec<(String, Vec<Step>)>,
}

impl FeatureBuilder {
    pub(super) fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            background: None,
            scenarios: Vec::new(),
        }
    }

    pub(super) fn with_background(mut self, steps: Vec<Step>) -> Self {
        self.background = Some(steps);
        self
    }

    pub(super) fn with_scenario(mut self, name: &str, steps: Vec<Step>) -> Self {
        self.scenarios.push((name.to_string(), steps));
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
            scenarios: self
                .scenarios
                .into_iter()
                .map(|(name, steps)| Scenario {
                    keyword: "Scenario".into(),
                    name,
                    description: None,
                    steps,
                    examples: Vec::new(),
                    tags: Vec::new(),
                    span: zero_span(),
                    position: zero_pos(),
                })
                .collect(),
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
    let ScenarioData { steps, .. } = extract_scenario_steps(&feature, scenario_index)
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
            .build();

        assert_eq!(feature.name, "demo");
        assert!(feature.background.is_some());
        assert_eq!(feature.scenarios.len(), 1);
        let Some(scenario) = feature.scenarios.first() else {
            panic!("expected one scenario");
        };
        assert_eq!(scenario.steps.len(), 1);
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
