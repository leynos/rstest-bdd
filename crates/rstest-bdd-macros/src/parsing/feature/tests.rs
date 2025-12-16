//! Tests for feature parsing utilities.

#[path = "missing_examples_tests.rs"]
mod missing_examples_tests;
#[path = "step_extraction_tests.rs"]
mod step_extraction_tests;
#[path = "support.rs"]
mod support;

use super::*;
use gherkin::StepType;
use rstest::rstest;

use support::{FeatureBuilder, StepBuilder};

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
#[case(
    "../rstest-bdd/tests/features/macros/does_not_exist.feature",
    "feature file not found"
)]
#[case(
    "../rstest-bdd/tests/features/macros/empty.feature",
    "failed to parse feature file"
)]
#[case("../rstest-bdd/tests/features/macros", "feature path is not a file")]
fn errors_when_feature_fails(#[case] rel_path: &str, #[case] expected_snippet: &str) {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(rel_path);
    let Err(err) = parse_and_load_feature(&path) else {
        panic!("expected failure for feature path: {rel_path}");
    };
    assert!(err.to_string().contains(expected_snippet));
}

#[test]
fn reports_requested_index_and_available_count_on_oob() {
    let feature = FeatureBuilder::new("demo").with_scenario(
        "only",
        vec![StepBuilder::new(StepType::Given, "step").build()],
    );

    let Err(err) = extract_scenario_steps(&feature.build(), Some(2)) else {
        panic!("expected scenario extraction to fail for out of range index");
    };

    let err = err.to_string();
    assert!(
        err.contains("scenario index out of range: 2 (available: 1)"),
        "error should report index and count, got: {err}",
    );
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
        "cached feature scenarios differ",
    );
    assert_eq!(
        first.scenarios.iter().map(|s| &s.name).collect::<Vec<_>>(),
        second.scenarios.iter().map(|s| &s.name).collect::<Vec<_>>(),
        "cached feature scenario names differ",
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
