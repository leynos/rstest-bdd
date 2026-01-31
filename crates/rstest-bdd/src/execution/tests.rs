//! Unit tests for the execution module.

use std::sync::Arc;

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
/// FIXME(#409): Remove this module when deprecated skip encoding functions are removed.
#[expect(
    deprecated,
    reason = "FIXME(#409): tests for deprecated skip encoding functions for backward compatibility"
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

#[test]
fn execution_error_skip_is_skip_returns_true() {
    let error = ExecutionError::Skip { message: None };
    assert!(error.is_skip());
}

#[test]
fn execution_error_skip_with_message_is_skip_returns_true() {
    let error = ExecutionError::Skip {
        message: Some("reason".into()),
    };
    assert!(error.is_skip());
}

#[test]
fn execution_error_step_not_found_is_skip_returns_false() {
    let error = ExecutionError::StepNotFound {
        index: 0,
        keyword: StepKeyword::Given,
        text: "missing".into(),
        feature_path: "test.feature".into(),
        scenario_name: "test".into(),
    };
    assert!(!error.is_skip());
}

#[test]
fn execution_error_missing_fixtures_is_skip_returns_false() {
    let error = ExecutionError::MissingFixtures(Arc::new(MissingFixturesDetails {
        step_pattern: "test step".into(),
        step_location: "test.rs:1".into(),
        required: vec!["fixture"],
        missing: vec!["fixture"],
        available: vec![],
        feature_path: "test.feature".into(),
        scenario_name: "test".into(),
    }));
    assert!(!error.is_skip());
}

#[test]
fn execution_error_handler_failed_is_skip_returns_false() {
    let error = ExecutionError::HandlerFailed {
        index: 0,
        keyword: StepKeyword::When,
        text: "failing step".into(),
        error: Arc::new(StepError::ExecutionError {
            pattern: "failing step".into(),
            function: "test_fn".into(),
            message: "boom".into(),
        }),
        feature_path: "test.feature".into(),
        scenario_name: "test".into(),
    };
    assert!(!error.is_skip());
}

#[test]
fn execution_error_skip_message_returns_none_for_skip_without_message() {
    let error = ExecutionError::Skip { message: None };
    assert_eq!(error.skip_message(), None);
}

#[test]
fn execution_error_skip_message_returns_message_for_skip_with_message() {
    let error = ExecutionError::Skip {
        message: Some("test reason".into()),
    };
    assert_eq!(error.skip_message(), Some("test reason"));
}

#[test]
fn execution_error_skip_message_returns_none_for_non_skip_errors() {
    let error = ExecutionError::StepNotFound {
        index: 0,
        keyword: StepKeyword::Given,
        text: "missing".into(),
        feature_path: "test.feature".into(),
        scenario_name: "test".into(),
    };
    assert_eq!(error.skip_message(), None);
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
