//! Unit tests for the execution module.

use rstest::rstest;

use super::{
    RuntimeMode, SKIP_NONE_PREFIX, SKIP_SOME_PREFIX, TestAttributeHint, decode_skip_message,
    encode_skip_message,
};

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

/// Verify that `RuntimeMode` and `TestAttributeHint` have matching variant counts.
///
/// This test serves as a compile-time-adjacent guard against enum drift. While Rust's
/// exhaustive pattern matching in `RuntimeMode::test_attribute_hint()` will catch
/// missing variants, this test provides explicit validation that both enums have
/// the same number of variants and that each `RuntimeMode` maps to a unique hint.
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
