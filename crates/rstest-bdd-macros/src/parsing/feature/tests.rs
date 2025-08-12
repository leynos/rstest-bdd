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

fn mk_step(ty: StepType, value: &str) -> Step {
    Step {
        keyword: kw(ty),
        ty,
        value: value.to_string(),
        docstring: None,
        table: None,
        span: Span { start: 0, end: 0 },
        position: LineCol { line: 0, col: 0 },
    }
}

fn mk_step_with_table(ty: StepType, value: &str, rows: Vec<Vec<&str>>) -> Step {
    Step {
        keyword: kw(ty),
        ty,
        value: value.to_string(),
        docstring: None,
        table: Some(gherkin::Table {
            rows: rows
                .into_iter()
                .map(|r| r.into_iter().map(str::to_string).collect())
                .collect(),
            span: Span { start: 0, end: 0 },
            position: LineCol { line: 0, col: 0 },
        }),
        span: Span { start: 0, end: 0 },
        position: LineCol { line: 0, col: 0 },
    }
}

fn mk_step_with_docstring(ty: StepType, value: &str, doc: &str) -> Step {
    Step {
        keyword: kw(ty),
        ty,
        value: value.to_string(),
        docstring: Some(doc.to_string()),
        table: None,
        span: Span { start: 0, end: 0 },
        position: LineCol { line: 0, col: 0 },
    }
}

#[test]
fn prepends_background_steps() {
    let feature = gherkin::Feature {
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
        ]
    );
}

#[test]
fn extracts_data_table() {
    let feature = gherkin::Feature {
        keyword: "Feature".into(),
        name: "example".into(),
        description: None,
        background: None,
        scenarios: vec![Scenario {
            keyword: "Scenario".into(),
            name: "table".into(),
            description: None,
            steps: vec![mk_step_with_table(
                StepType::Given,
                "numbers",
                vec![vec!["1", "2"], vec!["3", "4"]],
            )],
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
        vec![ParsedStep {
            keyword: rstest_bdd::StepKeyword::Given,
            text: "numbers".to_string(),
            docstring: None,
            table: Some(vec![
                vec!["1".to_string(), "2".to_string()],
                vec!["3".to_string(), "4".to_string()],
            ]),
        }]
    );
}

#[test]
fn extracts_docstring() {
    let feature = gherkin::Feature {
        keyword: "Feature".into(),
        name: "example".into(),
        description: None,
        background: None,
        scenarios: vec![Scenario {
            keyword: "Scenario".into(),
            name: "doc".into(),
            description: None,
            steps: vec![mk_step_with_docstring(
                StepType::Given,
                "text",
                "line1\nline2",
            )],
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
        vec![ParsedStep {
            keyword: rstest_bdd::StepKeyword::Given,
            text: "text".to_string(),
            docstring: Some("line1\nline2".to_string()),
            table: None,
        }]
    );
}

#[test]
fn background_steps_with_docstring_are_extracted() {
    let feature = gherkin::Feature {
        keyword: "Feature".into(),
        name: "example".into(),
        description: None,
        background: Some(Background {
            keyword: "Background".into(),
            name: String::new(),
            description: None,
            steps: vec![mk_step_with_docstring(
                StepType::Given,
                "setup",
                "bg line1\nbg line2",
            )],
            span: Span { start: 0, end: 0 },
            position: LineCol { line: 0, col: 0 },
        }),
        scenarios: vec![Scenario {
            keyword: "Scenario".into(),
            name: "run".into(),
            description: None,
            steps: vec![mk_step(StepType::When, "an action")],
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
        ]
    );
}
