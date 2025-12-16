//! Tests for scenario step extraction via `iter_parsed_steps_with_background`.
//!
//! This module verifies that background steps are correctly prepended to
//! scenario steps and that step payloads (data tables, docstrings, and keyword
//! synonyms) are preserved during extraction.
//!
//! These tests live separately from `tests.rs` to keep the entry-point test
//! module small, whilst still covering the richer step-extraction surface area.

use super::*;
use gherkin::StepType;
use rstest::rstest;

use super::support::{ExamplesBuilder, FeatureBuilder, StepBuilder, assert_feature_extraction};

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
            text: "a background step".to_string(),
            docstring: None,
            table: None,
            #[cfg(feature = "compile-time-validation")]
            span: proc_macro2::Span::call_site(),
        },
        ParsedStep {
            keyword: crate::StepKeyword::When,
            text: "an action".to_string(),
            docstring: None,
            table: None,
            #[cfg(feature = "compile-time-validation")]
            span: proc_macro2::Span::call_site(),
        },
        ParsedStep {
            keyword: crate::StepKeyword::Then,
            text: "a result".to_string(),
            docstring: None,
            table: None,
            #[cfg(feature = "compile-time-validation")]
            span: proc_macro2::Span::call_site(),
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
        #[cfg(feature = "compile-time-validation")]
        span: proc_macro2::Span::call_site(),
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
        #[cfg(feature = "compile-time-validation")]
        span: proc_macro2::Span::call_site(),
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
            #[cfg(feature = "compile-time-validation")]
            span: proc_macro2::Span::call_site(),
        },
        ParsedStep {
            keyword: crate::StepKeyword::When,
            text: "an action".to_string(),
            docstring: None,
            table: None,
            #[cfg(feature = "compile-time-validation")]
            span: proc_macro2::Span::call_site(),
        },
    ],
    None
)]
#[case::prepends_background_steps_for_scenario_outline(
    FeatureBuilder::new("example")
        .with_background(vec![StepBuilder::new(StepType::Given, "setup").build()])
        .with_scenario_outline(
            "outline",
            vec![
                StepBuilder::new(StepType::When, "an action").build(),
                StepBuilder::new(StepType::Then, "a result").build(),
            ],
            ExamplesBuilder::new()
                .with_table(vec![vec!["value"], vec!["1"]])
                .build(),
        ),
    vec![
        ParsedStep {
            keyword: crate::StepKeyword::Given,
            text: "setup".to_string(),
            docstring: None,
            table: None,
            #[cfg(feature = "compile-time-validation")]
            span: proc_macro2::Span::call_site(),
        },
        ParsedStep {
            keyword: crate::StepKeyword::When,
            text: "an action".to_string(),
            docstring: None,
            table: None,
            #[cfg(feature = "compile-time-validation")]
            span: proc_macro2::Span::call_site(),
        },
        ParsedStep {
            keyword: crate::StepKeyword::Then,
            text: "a result".to_string(),
            docstring: None,
            table: None,
            #[cfg(feature = "compile-time-validation")]
            span: proc_macro2::Span::call_site(),
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
            #[cfg(feature = "compile-time-validation")]
            span: proc_macro2::Span::call_site(),
        },
        ParsedStep {
            keyword: crate::StepKeyword::And,
            text: "second".into(),
            docstring: None,
            table: None,
            #[cfg(feature = "compile-time-validation")]
            span: proc_macro2::Span::call_site(),
        },
        ParsedStep {
            keyword: crate::StepKeyword::But,
            text: "negated".into(),
            docstring: None,
            table: None,
            #[cfg(feature = "compile-time-validation")]
            span: proc_macro2::Span::call_site(),
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
            #[cfg(feature = "compile-time-validation")]
            span: proc_macro2::Span::call_site(),
        },
        ParsedStep {
            keyword: crate::StepKeyword::Then,
            text: "result".into(),
            docstring: None,
            table: None,
            #[cfg(feature = "compile-time-validation")]
            span: proc_macro2::Span::call_site(),
        },
    ],
    None
)]
#[case::maps_leading_but_keyword(
    FeatureBuilder::new("example").with_scenario(
        "leading-but",
        vec![
            StepBuilder::new(StepType::When, "first")
                .with_keyword("But")
                .build(),
            StepBuilder::new(StepType::Then, "result").build(),
        ],
    ),
    vec![
        ParsedStep {
            keyword: crate::StepKeyword::But,
            text: "first".into(),
            docstring: None,
            table: None,
            #[cfg(feature = "compile-time-validation")]
            span: proc_macro2::Span::call_site(),
        },
        ParsedStep {
            keyword: crate::StepKeyword::Then,
            text: "result".into(),
            docstring: None,
            table: None,
            #[cfg(feature = "compile-time-validation")]
            span: proc_macro2::Span::call_site(),
        },
    ],
    None
)]
#[case::maps_mixed_keyword_sequence(
    FeatureBuilder::new("example").with_scenario(
        "mixed",
        vec![
            StepBuilder::new(StepType::Given, "start").build(),
            StepBuilder::new(StepType::Given, "cont")
                .with_keyword("And")
                .build(),
            StepBuilder::new(StepType::Given, "neg")
                .with_keyword("But")
                .build(),
            StepBuilder::new(StepType::Then, "end").build(),
        ],
    ),
    vec![
        ParsedStep {
            keyword: crate::StepKeyword::Given,
            text: "start".into(),
            docstring: None,
            table: None,
            #[cfg(feature = "compile-time-validation")]
            span: proc_macro2::Span::call_site(),
        },
        ParsedStep {
            keyword: crate::StepKeyword::And,
            text: "cont".into(),
            docstring: None,
            table: None,
            #[cfg(feature = "compile-time-validation")]
            span: proc_macro2::Span::call_site(),
        },
        ParsedStep {
            keyword: crate::StepKeyword::But,
            text: "neg".into(),
            docstring: None,
            table: None,
            #[cfg(feature = "compile-time-validation")]
            span: proc_macro2::Span::call_site(),
        },
        ParsedStep {
            keyword: crate::StepKeyword::Then,
            text: "end".into(),
            docstring: None,
            table: None,
            #[cfg(feature = "compile-time-validation")]
            span: proc_macro2::Span::call_site(),
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
