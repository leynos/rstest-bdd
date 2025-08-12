//! Tests for feature parsing utilities.

use super::*;
use gherkin::{Background, LineCol, Scenario, Span, Step, StepType};

#[expect(
    unreachable_patterns,
    reason = "StepType currently only has three variants"
)]
fn kw(ty: StepType) -> String {
    match ty {
        StepType::Given => "Given",
        StepType::When => "When",
        StepType::Then => "Then",
        // Ensure tests fail loudly if an unsupported keyword is passed.
        _ => unreachable!("kw() only supports Given, When, and Then"),
    }
    .to_string()
}

struct StepBuilder {
    ty: StepType,
    value: String,
    docstring: Option<String>,
    table: Option<gherkin::Table>,
}

impl StepBuilder {
    fn new(ty: StepType, value: &str) -> Self {
        Self {
            ty,
            value: value.to_string(),
            docstring: None,
            table: None,
        }
    }

    fn with_docstring(mut self, doc: &str) -> Self {
        self.docstring = Some(doc.to_string());
        self
    }

    fn with_table(mut self, rows: Vec<Vec<&str>>) -> Self {
        self.table = Some(gherkin::Table {
            rows: rows
                .into_iter()
                .map(|r| r.into_iter().map(str::to_string).collect())
                .collect(),
            span: Span { start: 0, end: 0 },
            position: LineCol { line: 0, col: 0 },
        });
        self
    }

    fn build(self) -> Step {
        Step {
            keyword: kw(self.ty),
            ty: self.ty,
            value: self.value,
            docstring: self.docstring,
            table: self.table,
            span: Span { start: 0, end: 0 },
            position: LineCol { line: 0, col: 0 },
        }
    }
}

struct FeatureBuilder {
    name: String,
    background: Option<Vec<Step>>,
    scenarios: Vec<(String, Vec<Step>)>,
}

impl FeatureBuilder {
    fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            background: None,
            scenarios: Vec::new(),
        }
    }

    fn with_background(mut self, steps: Vec<Step>) -> Self {
        self.background = Some(steps);
        self
    }

    fn with_scenario(mut self, name: &str, steps: Vec<Step>) -> Self {
        self.scenarios.push((name.to_string(), steps));
        self
    }

    fn build(self) -> gherkin::Feature {
        gherkin::Feature {
            keyword: "Feature".into(),
            name: self.name,
            description: None,
            background: self.background.map(|steps| Background {
                keyword: "Background".into(),
                name: String::new(),
                description: None,
                steps,
                span: Span { start: 0, end: 0 },
                position: LineCol { line: 0, col: 0 },
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
                    span: Span { start: 0, end: 0 },
                    position: LineCol { line: 0, col: 0 },
                })
                .collect(),
            rules: Vec::new(),
            tags: Vec::new(),
            span: Span { start: 0, end: 0 },
            position: LineCol { line: 0, col: 0 },
            path: None,
        }
    }
}

fn assert_extracted_steps(feature: &gherkin::Feature, expected: &[ParsedStep]) {
    let ScenarioData { steps, .. } = extract_scenario_steps(feature, Some(0))
        .unwrap_or_else(|_| panic!("scenario extraction failed"));
    assert_eq!(steps, expected);
}

#[test]
fn prepends_background_steps() {
    let feature = FeatureBuilder::new("example")
        .with_background(vec![
            StepBuilder::new(StepType::Given, "a background step").build(),
        ])
        .with_scenario(
            "run",
            vec![
                StepBuilder::new(StepType::When, "an action").build(),
                StepBuilder::new(StepType::Then, "a result").build(),
            ],
        )
        .build();

    assert_extracted_steps(
        &feature,
        &[
            ParsedStep {
                keyword: rstest_bdd::StepKeyword::Given,
                text: "a background step".to_string(),
                docstring: None,
                table: None,
            },
            ParsedStep {
                keyword: rstest_bdd::StepKeyword::When,
                text: "an action".to_string(),
                docstring: None,
                table: None,
            },
            ParsedStep {
                keyword: rstest_bdd::StepKeyword::Then,
                text: "a result".to_string(),
                docstring: None,
                table: None,
            },
        ],
    );
}

#[test]
fn extracts_data_table() {
    let feature = FeatureBuilder::new("example")
        .with_scenario(
            "table",
            vec![
                StepBuilder::new(StepType::Given, "numbers")
                    .with_table(vec![vec!["1", "2"], vec!["3", "4"]])
                    .build(),
            ],
        )
        .build();

    assert_extracted_steps(
        &feature,
        &[ParsedStep {
            keyword: rstest_bdd::StepKeyword::Given,
            text: "numbers".to_string(),
            docstring: None,
            table: Some(vec![
                vec!["1".to_string(), "2".to_string()],
                vec!["3".to_string(), "4".to_string()],
            ]),
        }],
    );
}

#[test]
fn extracts_docstring() {
    let feature = FeatureBuilder::new("example")
        .with_scenario(
            "doc",
            vec![
                StepBuilder::new(StepType::Given, "text")
                    .with_docstring("line1\nline2")
                    .build(),
            ],
        )
        .build();

    assert_extracted_steps(
        &feature,
        &[ParsedStep {
            keyword: rstest_bdd::StepKeyword::Given,
            text: "text".to_string(),
            docstring: Some("line1\nline2".to_string()),
            table: None,
        }],
    );
}

#[test]
fn background_steps_with_docstring_are_extracted() {
    let feature = FeatureBuilder::new("example")
        .with_background(vec![
            StepBuilder::new(StepType::Given, "setup")
                .with_docstring("bg line1\nbg line2")
                .build(),
        ])
        .with_scenario(
            "run",
            vec![StepBuilder::new(StepType::When, "an action").build()],
        )
        .build();

    assert_extracted_steps(
        &feature,
        &[
            ParsedStep {
                keyword: rstest_bdd::StepKeyword::Given,
                text: "setup".to_string(),
                docstring: Some("bg line1\nbg line2".to_string()),
                table: None,
            },
            ParsedStep {
                keyword: rstest_bdd::StepKeyword::When,
                text: "an action".to_string(),
                docstring: None,
                table: None,
            },
        ],
    );
}
