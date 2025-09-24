//! Tests for feature parsing utilities.

use super::*;
use gherkin::StepType;
use rstest::rstest;

#[path = "test_support.rs"]
mod test_support;

use test_support::{FeatureBuilder, StepBuilder, assert_feature_extraction};

#[rstest]
#[case("And", StepType::Given, crate::StepKeyword::And)]
#[case("AND", StepType::Then, crate::StepKeyword::And)]
#[case(" and  ", StepType::When, crate::StepKeyword::And)]
#[case("But", StepType::Given, crate::StepKeyword::But)]
#[case("BUT", StepType::Then, crate::StepKeyword::But)]
#[case(" but ", StepType::When, crate::StepKeyword::But)]
#[case("Given", StepType::Given, crate::StepKeyword::Given)]
fn parses_step_keyword_variants(
    #[case] kw: &str,
    #[case] ty: StepType,
    #[case] expected: crate::StepKeyword,
) {
    assert_eq!(parse_step_keyword(kw, ty), expected);
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
                .with_docstring("line1
line2")
                .build(),
        ],
    ),
    vec![ParsedStep {
        keyword: crate::StepKeyword::Given,
        text: "text".to_string(),
        docstring: Some("line1
line2".to_string()),
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
                .with_docstring("bg line1
bg line2")
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
            docstring: Some("bg line1
bg line2".to_string()),
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

#[expect(
    clippy::expect_used,
    reason = "test asserts cache behaviour; panics simplify failures"
)]
#[test]
fn caches_features_by_path() {
    use std::io::Write;
    use tempfile::NamedTempFile;
    super::clear_feature_cache();
    let mut tf = NamedTempFile::new().expect("create temp feature");
    write!(
        tf,
        "Feature: cache
Scenario: demo
  Given step
"
    )
    .expect("write feature");
    let path = tf.path().to_path_buf();
    let first = parse_and_load_feature(&path).expect("first parse");
    // Close deletes the file; cached read must still succeed
    tf.close().expect("close temp feature");
    let second = parse_and_load_feature(&path).expect("cached parse");
    assert_eq!(first.name, second.name, "cached feature name differs");
    assert_eq!(
        first.scenarios.len(),
        second.scenarios.len(),
        "cached feature scenarios differ"
    );
    assert_eq!(
        first.scenarios.iter().map(|s| &s.name).collect::<Vec<_>>(),
        second.scenarios.iter().map(|s| &s.name).collect::<Vec<_>>(),
        "cached feature scenario names differ"
    );
}

#[cfg(feature = "compile-time-validation")]
#[test]
/// `ParsedStep` equality ignores span differences.
fn parsed_step_equality_ignores_span() {
    let a = ParsedStep {
        keyword: crate::StepKeyword::Given,
        text: "step".into(),
        docstring: None,
        table: None,
        #[cfg(feature = "compile-time-validation")]
        span: proc_macro2::Span::call_site(),
    };
    let mut b = a.clone();
    b.span = proc_macro2::Span::mixed_site();
    assert_eq!(a, b, "spans differ but equality should ignore them");

    let c = ParsedStep {
        keyword: crate::StepKeyword::When,
        ..a
    };
    assert_ne!(b, c, "different keywords must not be equal");
}
