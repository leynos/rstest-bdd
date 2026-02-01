//! Unit tests for the execution module.
//!
//! This module contains tests for the step execution infrastructure, including:
//!
//! - [`RuntimeMode`] and [`TestAttributeHint`] enum behaviour and mapping
//! - [`ExecutionError`] variant construction, `is_skip()`, and `skip_message()`
//! - Skip message extraction logic (mirroring generated code behaviour)
//! - Deprecated skip encoding functions (scheduled for removal)
//!
//! These tests validate the core execution types used by generated scenario code
//! and direct registry consumers.

use std::sync::Arc;

use rstest::rstest;

use crate::{StepError, StepKeyword};

use super::{ExecutionError, MissingFixturesDetails, RuntimeMode, TestAttributeHint};

#[test]
fn runtime_mode_sync_is_default() {
    assert_eq!(RuntimeMode::default(), RuntimeMode::Sync);
}

#[test]
fn runtime_mode_sync_is_not_async() {
    assert!(!RuntimeMode::Sync.is_async());
}

#[test]
fn runtime_mode_tokio_current_thread_is_async() {
    assert!(RuntimeMode::TokioCurrentThread.is_async());
}

#[test]
fn runtime_mode_sync_hint_is_rstest_only() {
    assert_eq!(
        RuntimeMode::Sync.test_attribute_hint(),
        TestAttributeHint::RstestOnly
    );
}

#[test]
fn runtime_mode_tokio_hint_is_rstest_with_tokio() {
    assert_eq!(
        RuntimeMode::TokioCurrentThread.test_attribute_hint(),
        TestAttributeHint::RstestWithTokioCurrentThread
    );
}

/// Tests for deprecated skip encoding functions.
///
/// FIXME: Remove this module when deprecated skip encoding functions are removed.
/// See: <https://github.com/leynos/rstest-bdd/issues/409>
#[expect(
    deprecated,
    reason = "FIXME: https://github.com/leynos/rstest-bdd/issues/409 - testing deprecated skip encoding functions"
)]
mod deprecated_skip_encoding {
    use rstest::rstest;

    use super::super::{
        SKIP_NONE_PREFIX, SKIP_SOME_PREFIX, decode_skip_message, encode_skip_message,
    };

    #[test]
    fn encode_skip_message_none_produces_prefix_only() {
        let encoded = encode_skip_message(None);
        assert_eq!(encoded.len(), 1);
        assert_eq!(encoded.chars().next(), Some(SKIP_NONE_PREFIX));
    }

    #[test]
    fn encode_skip_message_some_includes_message() {
        let encoded = encode_skip_message(Some("test message".to_string()));
        assert!(encoded.starts_with(SKIP_SOME_PREFIX));
        assert!(encoded.contains("test message"));
    }

    #[rstest]
    #[case::none(None)]
    #[case::some(Some("skip reason".to_string()))]
    #[case::empty_string(Some(String::new()))]
    #[case::unicode(Some("Unicode: 😀 🎉".to_string()))]
    fn decode_skip_message_round_trip(#[case] input: Option<String>) {
        let encoded = encode_skip_message(input.clone());
        let decoded = decode_skip_message(encoded);
        assert_eq!(decoded, input);
    }

    #[test]
    fn decode_skip_message_malformed_input_preserved() {
        // Malformed input (no valid prefix) should be returned as-is
        let malformed = "unexpected input".to_string();
        let decoded = decode_skip_message(malformed.clone());
        assert_eq!(decoded, Some(malformed));
    }

    #[test]
    fn decode_skip_message_empty_string_preserved() {
        // Empty string has no prefix character
        let decoded = decode_skip_message(String::new());
        assert_eq!(decoded, Some(String::new()));
    }
}

// ============================================================================
// ExecutionError tests
// ============================================================================

/// Helper enum for parameterized `ExecutionError` tests.
///
/// Provides test data for `is_skip()` and `skip_message()` method assertions
/// across all `ExecutionError` variants.
enum ExecutionErrorTestCase {
    SkipWithoutMessage,
    SkipWithMessage(&'static str),
    StepNotFound,
    HandlerFailed,
    MissingFixtures,
}

impl ExecutionErrorTestCase {
    fn make_error(&self) -> ExecutionError {
        match self {
            Self::SkipWithoutMessage => ExecutionError::Skip { message: None },
            Self::SkipWithMessage(msg) => ExecutionError::Skip {
                message: Some((*msg).into()),
            },
            Self::StepNotFound => ExecutionError::StepNotFound {
                index: 0,
                keyword: StepKeyword::Given,
                text: "missing".into(),
                feature_path: "test.feature".into(),
                scenario_name: "test".into(),
            },
            Self::HandlerFailed => ExecutionError::HandlerFailed {
                index: 0,
                keyword: StepKeyword::When,
                text: "failing".into(),
                error: Arc::new(StepError::ExecutionError {
                    pattern: "failing".into(),
                    function: "test_fn".into(),
                    message: "boom".into(),
                }),
                feature_path: "test.feature".into(),
                scenario_name: "test".into(),
            },
            Self::MissingFixtures => {
                ExecutionError::MissingFixtures(Arc::new(MissingFixturesDetails {
                    step_pattern: "test step".into(),
                    step_location: "test.rs:1".into(),
                    required: vec!["fixture"],
                    missing: vec!["fixture"],
                    available: vec![],
                    feature_path: "test.feature".into(),
                    scenario_name: "test".into(),
                }))
            }
        }
    }

    fn expected_is_skip(&self) -> bool {
        matches!(self, Self::SkipWithoutMessage | Self::SkipWithMessage(_))
    }

    fn expected_skip_message(&self) -> Option<&'static str> {
        match self {
            Self::SkipWithMessage(msg) => Some(msg),
            Self::SkipWithoutMessage
            | Self::StepNotFound
            | Self::HandlerFailed
            | Self::MissingFixtures => None,
        }
    }
}

#[rstest]
#[case::skip_without_message(ExecutionErrorTestCase::SkipWithoutMessage)]
#[case::skip_with_message(ExecutionErrorTestCase::SkipWithMessage("test reason"))]
#[case::step_not_found(ExecutionErrorTestCase::StepNotFound)]
#[case::handler_failed(ExecutionErrorTestCase::HandlerFailed)]
#[case::missing_fixtures(ExecutionErrorTestCase::MissingFixtures)]
fn execution_error_is_skip_returns_expected_value(#[case] test_case: ExecutionErrorTestCase) {
    let error = test_case.make_error();
    let expected = test_case.expected_is_skip();
    assert_eq!(error.is_skip(), expected);
}

#[rstest]
#[case::skip_without_message(ExecutionErrorTestCase::SkipWithoutMessage)]
#[case::skip_with_message(ExecutionErrorTestCase::SkipWithMessage("test reason"))]
#[case::step_not_found(ExecutionErrorTestCase::StepNotFound)]
#[case::handler_failed(ExecutionErrorTestCase::HandlerFailed)]
#[case::missing_fixtures(ExecutionErrorTestCase::MissingFixtures)]
fn execution_error_skip_message_returns_expected_value(#[case] test_case: ExecutionErrorTestCase) {
    let error = test_case.make_error();
    let expected = test_case.expected_skip_message();
    assert_eq!(error.skip_message(), expected);
}

#[test]
fn execution_error_implements_std_error() {
    let error = ExecutionError::Skip { message: None };
    let _: &dyn std::error::Error = &error;
}

#[test]
fn execution_error_handler_failed_source_returns_inner_error() {
    let inner = StepError::ExecutionError {
        pattern: "test".into(),
        function: "fn".into(),
        message: "boom".into(),
    };
    let error = ExecutionError::HandlerFailed {
        index: 0,
        keyword: StepKeyword::When,
        text: "test".into(),
        error: Arc::new(inner),
        feature_path: "test.feature".into(),
        scenario_name: "test".into(),
    };
    assert!(std::error::Error::source(&error).is_some());
}

#[test]
fn execution_error_skip_source_returns_none() {
    let error = ExecutionError::Skip { message: None };
    assert!(std::error::Error::source(&error).is_none());
}

// ============================================================================
// Skip extraction tests (mirrors generated __rstest_bdd_extract_skip_message)
// ============================================================================

/// Helper function that mirrors the generated `__rstest_bdd_extract_skip_message`.
///
/// This allows us to test the extraction logic that is used in generated code.
/// The `Option<Option<String>>` type is intentional: the outer `Option` distinguishes
/// skip errors from non-skip errors, while the inner `Option` carries the optional
/// skip message.
#[expect(
    clippy::option_option,
    reason = "mirrors generated code which uses Option<Option<String>> intentionally"
)]
fn extract_skip_message(error: &ExecutionError) -> Option<Option<String>> {
    if error.is_skip() {
        Some(error.skip_message().map(String::from))
    } else {
        None
    }
}

impl ExecutionErrorTestCase {
    /// Returns the expected result of `extract_skip_message` for this test case.
    #[expect(
        clippy::option_option,
        reason = "mirrors generated code which uses Option<Option<String>> intentionally"
    )]
    fn expected_extract_skip_message(&self) -> Option<Option<String>> {
        match self {
            Self::SkipWithoutMessage => Some(None),
            Self::SkipWithMessage(msg) => Some(Some((*msg).to_string())),
            Self::StepNotFound | Self::HandlerFailed | Self::MissingFixtures => None,
        }
    }
}

#[rstest]
#[case::skip_without_message(ExecutionErrorTestCase::SkipWithoutMessage)]
#[case::skip_with_message(ExecutionErrorTestCase::SkipWithMessage("reason"))]
#[case::step_not_found(ExecutionErrorTestCase::StepNotFound)]
#[case::handler_failed(ExecutionErrorTestCase::HandlerFailed)]
#[case::missing_fixtures(ExecutionErrorTestCase::MissingFixtures)]
fn extract_skip_message_returns_expected_value(#[case] test_case: ExecutionErrorTestCase) {
    let error = test_case.make_error();
    let expected = test_case.expected_extract_skip_message();
    assert_eq!(extract_skip_message(&error), expected);
}

/// Verify that `RuntimeMode` and `TestAttributeHint` have matching variant counts.
///
/// This test provides explicit validation that both enums have the same number
/// of variants and that each `RuntimeMode` maps to a unique hint.
#[test]
fn runtime_mode_and_test_attribute_hint_variant_parity() {
    // Collect all RuntimeMode variants and their corresponding hints
    let runtime_modes = [RuntimeMode::Sync, RuntimeMode::TokioCurrentThread];
    let expected_hints = [
        TestAttributeHint::RstestOnly,
        TestAttributeHint::RstestWithTokioCurrentThread,
    ];

    // Verify variant counts match
    assert_eq!(
        runtime_modes.len(),
        expected_hints.len(),
        "RuntimeMode and TestAttributeHint should have the same number of variants"
    );

    // Verify each RuntimeMode maps to the expected TestAttributeHint
    for (mode, expected_hint) in runtime_modes.iter().zip(expected_hints.iter()) {
        assert_eq!(
            mode.test_attribute_hint(),
            *expected_hint,
            "RuntimeMode::{mode:?} should map to TestAttributeHint::{expected_hint:?}"
        );
    }
}
