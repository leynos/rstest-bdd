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
            (
                rstest_bdd::StepKeyword::Given,
                "a background step".to_string()
            ),
            (rstest_bdd::StepKeyword::When, "an action".to_string()),
            (rstest_bdd::StepKeyword::Then, "a result".to_string()),
        ]
    );
}
