//! Tests for feature parsing utilities.

use super::*;
use gherkin::{Background, LineCol, Scenario, Span, Step, StepType};
use rstest::rstest;

// This `#[expect]` triggers if `gherkin::StepType` adds variants so we update
// `kw()` and `From<StepType> for StepKeyword`.
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

struct StepBuilder {
    ty: StepType,
    value: String,
    docstring: Option<String>,
    table: Option<gherkin::Table>,
    keyword: Option<String>,
}

impl StepBuilder {
    fn new(ty: StepType, value: &str) -> Self {
        Self {
            ty,
            value: value.to_string(),
            docstring: None,
            table: None,
            keyword: None,
        }
    }

    fn with_keyword(mut self, kw: &str) -> Self {
        self.keyword = Some(kw.to_string());
        self
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
            keyword: self.keyword.unwrap_or_else(|| kw(self.ty)),
            ty: self.ty,
            value: self.value,
            docstring: self.docstring,
            table: self.table,
            span: Span { start: 0, end: 0 },
            position: LineCol { line: 0, col: 0 },
        }
    }
}

// gherkin normalises "And"/"But" based on `ty`, but the parser under test
// uses the `keyword` string instead. Hard-code `ty` to `Given` so tests can
// verify keyword resolution without upstream interference.
fn raw_step(keyword: &str, value: &str) -> Step {
    // Intentionally set `ty` to `Given` so tests can pass raw
    // "And"/"But" via `keyword` without StepBuilder's auto-mapping
    // altering them. The parser under test uses the `keyword` string
    // (case-insensitive) and ignores `ty`.
    Step {
        keyword: keyword.to_string(),
        ty: StepType::Given,
        value: value.to_string(),
        docstring: None,
        table: None,
        span: Span { start: 0, end: 0 },
        position: LineCol { line: 0, col: 0 },
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

// Build a feature from the provided builder, extract the steps for the scenario
// at `scenario_index`, and assert that they match `expected_steps`.
fn assert_feature_extraction(
    feature_builder: FeatureBuilder,
    expected_steps: &[ParsedStep],
    scenario_index: Option<usize>,
) {
    let feature = feature_builder.build();
    let ScenarioData { steps, .. } = extract_scenario_steps(&feature, scenario_index)
        .unwrap_or_else(|_| panic!("failed to extract scenario steps at index {scenario_index:?}"));
    assert_eq!(
        steps, expected_steps,
        "extracted steps did not match expectation"
    );
}

#[rstest]
#[case::prepends_background_steps(
    FeatureBuilder::new("example")
        .with_background(vec![StepBuilder::new(StepType::Given, "a background step").build()])
        .with_scenario(
            "run",
            vec![
                StepBuilder::new(StepType::When, "an action").build(),
                StepBuilder::new(StepType::Then, "a result").build(),
            ],
        ),
    vec![
        ParsedStep {
            keyword: crate::StepKeyword::Given,
<<<<<<< HEAD
            text: "a background step".to_string(),
            docstring: None,
            table: None,
        },
        ParsedStep {
            keyword: crate::StepKeyword::When,
            text: "an action".to_string(),
            docstring: None,
||||||| parent of ce4ff4d (Return Result from map_step and reset keyword context)
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
fn normalises_and_but_to_previous_keyword() {
    use gherkin::{LineCol, Span};

    fn raw_step(keyword: &str, value: &str) -> Step {
        Step {
            keyword: keyword.to_string(),
            ty: StepType::Given,
            value: value.to_string(),
            docstring: None,
            table: None,
            span: Span { start: 0, end: 0 },
            position: LineCol { line: 0, col: 0 },
        }
    }

    let steps = vec![
        StepBuilder::new(StepType::Given, "precondition").build(),
        raw_step("And", "another"),
        raw_step("But", "exception"),
        StepBuilder::new(StepType::When, "action").build(),
        raw_step("And", "also action"),
        StepBuilder::new(StepType::Then, "result").build(),
    ];

    let feature = FeatureBuilder::new("example")
        .with_scenario("case", steps)
        .build();

    assert_extracted_steps(
        &feature,
        &[
            ParsedStep {
                keyword: crate::StepKeyword::Given,
                text: "precondition".to_string(),
                docstring: None,
                table: None,
            },
            ParsedStep {
                keyword: crate::StepKeyword::Given,
                text: "another".to_string(),
                docstring: None,
                table: None,
            },
            ParsedStep {
                keyword: crate::StepKeyword::Given,
                text: "exception".to_string(),
                docstring: None,
                table: None,
            },
            ParsedStep {
                keyword: crate::StepKeyword::When,
                text: "action".to_string(),
                docstring: None,
                table: None,
            },
            ParsedStep {
                keyword: crate::StepKeyword::When,
                text: "also action".to_string(),
                docstring: None,
                table: None,
            },
            ParsedStep {
                keyword: crate::StepKeyword::Then,
                text: "result".to_string(),
                docstring: None,
                table: None,
            },
        ],
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
            keyword: crate::StepKeyword::Given,
            text: "text".to_string(),
            docstring: Some("line1\nline2".to_string()),
=======
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
fn normalises_and_but_to_previous_keyword() {
    let steps = vec![
        StepBuilder::new(StepType::Given, "precondition").build(),
        raw_step("And", "another"),
        raw_step("But", "exception"),
        StepBuilder::new(StepType::When, "action").build(),
        raw_step("And", "also action"),
        StepBuilder::new(StepType::Then, "result").build(),
    ];

    let feature = FeatureBuilder::new("example")
        .with_scenario("case", steps)
        .build();

    assert_extracted_steps(
        &feature,
        &[
            ParsedStep {
                keyword: crate::StepKeyword::Given,
                text: "precondition".to_string(),
                docstring: None,
                table: None,
            },
            ParsedStep {
                keyword: crate::StepKeyword::Given,
                text: "another".to_string(),
                docstring: None,
                table: None,
            },
            ParsedStep {
                keyword: crate::StepKeyword::Given,
                text: "exception".to_string(),
                docstring: None,
                table: None,
            },
            ParsedStep {
                keyword: crate::StepKeyword::When,
                text: "action".to_string(),
                docstring: None,
                table: None,
            },
            ParsedStep {
                keyword: crate::StepKeyword::When,
                text: "also action".to_string(),
                docstring: None,
                table: None,
            },
            ParsedStep {
                keyword: crate::StepKeyword::Then,
                text: "result".to_string(),
                docstring: None,
                table: None,
            },
        ],
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
            keyword: crate::StepKeyword::Given,
            text: "text".to_string(),
            docstring: Some("line1\nline2".to_string()),
>>>>>>> ce4ff4d (Return Result from map_step and reset keyword context)
            table: None,
        },
        ParsedStep {
            keyword: crate::StepKeyword::Then,
            text: "a result".to_string(),
            docstring: None,
            table: None,
        },
    ],
    None
)]
#[case::extracts_data_table(
    FeatureBuilder::new("example").with_scenario(
        "table",
        vec![
            StepBuilder::new(StepType::Given, "numbers")
                .with_table(vec![vec!["1", "2"], vec!["3", "4"]])
                .build(),
        ],
    ),
    vec![ParsedStep {
        keyword: crate::StepKeyword::Given,
        text: "numbers".to_string(),
        docstring: None,
        table: Some(vec![
            vec!["1".to_string(), "2".to_string()],
            vec!["3".to_string(), "4".to_string()],
        ]),
    }],
    None
)]
#[case::extracts_docstring(
    FeatureBuilder::new("example").with_scenario(
        "doc",
        vec![
            StepBuilder::new(StepType::Given, "text")
                .with_docstring("line1\nline2")
                .build(),
        ],
    ),
    vec![ParsedStep {
        keyword: crate::StepKeyword::Given,
        text: "text".to_string(),
        docstring: Some("line1\nline2".to_string()),
        table: None,
    }],
    None
)]
#[case::background_steps_with_docstring_are_extracted(
    FeatureBuilder::new("example")
        .with_background(vec![
            StepBuilder::new(StepType::Given, "setup")
                .with_docstring("bg line1\nbg line2")
                .build(),
        ])
        .with_scenario(
            "run",
            vec![StepBuilder::new(StepType::When, "an action").build()],
        ),
    vec![
        ParsedStep {
            keyword: crate::StepKeyword::Given,
            text: "setup".to_string(),
            docstring: Some("bg line1\nbg line2".to_string()),
            table: None,
        },
        ParsedStep {
            keyword: crate::StepKeyword::When,
            text: "an action".to_string(),
            docstring: None,
            table: None,
        },
    ],
    None
)]
#[case::maps_and_and_but_keywords(
    FeatureBuilder::new("example").with_scenario(
        "synonyms",
        vec![
            StepBuilder::new(StepType::When, "first").build(),
            StepBuilder::new(StepType::When, "second")
                .with_keyword("And")
                .build(),
            StepBuilder::new(StepType::Then, "negated")
                .with_keyword("But")
                .build(),
        ],
    ),
    vec![
        ParsedStep {
            keyword: crate::StepKeyword::When,
            text: "first".into(),
            docstring: None,
            table: None,
        },
        ParsedStep {
            keyword: crate::StepKeyword::And,
            text: "second".into(),
            docstring: None,
            table: None,
        },
        ParsedStep {
            keyword: crate::StepKeyword::But,
            text: "negated".into(),
            docstring: None,
            table: None,
        },
    ],
    None
)]
#[case::maps_leading_and_keyword(
    FeatureBuilder::new("example").with_scenario(
        "leading-and",
        vec![
            StepBuilder::new(StepType::When, "first")
                .with_keyword("And")
                .build(),
            StepBuilder::new(StepType::Then, "result").build(),
        ],
    ),
    vec![
        ParsedStep {
            keyword: crate::StepKeyword::And,
            text: "first".into(),
            docstring: None,
            table: None,
        },
        ParsedStep {
            keyword: crate::StepKeyword::Then,
            text: "result".into(),
            docstring: None,
            table: None,
        },
    ],
    None
)]
fn extracts_scenario_steps(
    #[case] feature: FeatureBuilder,
    #[case] expected: Vec<ParsedStep>,
    #[case] index: Option<usize>,
) {
    assert_feature_extraction(feature, &expected, index);
}

#[test]
fn rejects_leading_and_after_background() {
    let feature = FeatureBuilder::new("example")
        .with_background(vec![StepBuilder::new(StepType::Given, "setup").build()])
        .with_scenario("case", vec![raw_step("And", "continuation")])
        .build();

    assert!(extract_scenario_steps(&feature, Some(0)).is_err());
}

#[rstest]
#[case("And")]
#[case("But")]
fn rejects_leading_conjunction_without_background(#[case] kw: &str) {
    // A Scenario that starts with And/But should be rejected because there is
    // no preceding primary keyword to inherit from.
    let feature = FeatureBuilder::new("example")
        .with_scenario("case", vec![raw_step(kw, "start")])
        .build();

    assert!(extract_scenario_steps(&feature, Some(0)).is_err());
}

#[rstest]
#[case("tests/features/does_not_exist.feature", "feature file not found")]
#[case("tests/features/empty.feature", "failed to parse feature file")]
#[case("tests/features", "feature path is not a file")]
fn errors_when_feature_fails(#[case] rel_path: &str, #[case] expected_snippet: &str) {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(rel_path);
    let Err(err) = parse_and_load_feature(&path) else {
        panic!("expected failure for feature path: {rel_path}");
    };
    assert!(err.to_string().contains(expected_snippet));
}

