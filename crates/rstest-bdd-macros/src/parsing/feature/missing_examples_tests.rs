//! Behavioural tests covering scenario outlines without Examples tables.

use super::*;
use gherkin::{Feature, LineCol, Scenario, Span};

#[expect(
    clippy::expect_used,
    reason = "tests assert error paths; panics surface unexpected success"
)]
#[test]
fn scenario_outline_missing_examples_surfaces_scenario_name() {
    let scenario_name = "outline without examples";
    let feature = Feature {
        keyword: "Feature".into(),
        name: "missing examples feature".into(),
        description: None,
        background: None,
        scenarios: vec![Scenario {
            keyword: "Scenario Outline".into(),
            name: scenario_name.into(),
            description: None,
            steps: Vec::new(),
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

    let err = extract_scenario_steps(&feature, None)
        .expect_err("scenario outline without examples should error");

    let message = err.to_string();
    assert!(
        message.contains("Scenario Outline missing Examples table"),
        "error should mention missing examples; got: {message}",
    );
    assert!(
        message.contains(scenario_name),
        "error should include scenario name; got: {message}",
    );
}
