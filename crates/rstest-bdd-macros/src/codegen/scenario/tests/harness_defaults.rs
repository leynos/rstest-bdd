//! Tests for harness-led default attribute-policy precedence.

use super::{RuntimeMode, generate_test_attrs};

#[expect(clippy::expect_used, reason = "test helper with descriptive failures")]
fn parse_path(s: &str) -> syn::Path {
    syn::parse_str::<syn::Path>(s).expect("valid path")
}

#[rstest::rstest]
#[case::tokio_harness_beats_sync_runtime(
    RuntimeMode::Sync,
    Some(parse_path("rstest_bdd_harness_tokio::TokioHarness")),
    None,
    true,
    false
)]
#[case::gpui_harness_beats_tokio_runtime(
    RuntimeMode::TokioCurrentThread,
    Some(parse_path("rstest_bdd_harness_gpui::GpuiHarness")),
    None,
    false,
    true
)]
#[case::std_harness_beats_tokio_runtime(
    RuntimeMode::TokioCurrentThread,
    Some(parse_path("rstest_bdd_harness::StdHarness")),
    None,
    false,
    false
)]
#[case::unknown_harness_falls_back_to_runtime(
    RuntimeMode::TokioCurrentThread,
    Some(parse_path("my::Harness")),
    None,
    true,
    false
)]
#[case::explicit_unknown_attributes_override_known_harness(
    RuntimeMode::TokioCurrentThread,
    Some(parse_path("rstest_bdd_harness_tokio::TokioHarness")),
    Some(parse_path("my::Policy")),
    false,
    false
)]
#[case::explicit_attributes_override_known_harness(
    RuntimeMode::Sync,
    Some(parse_path("rstest_bdd_harness_gpui::GpuiHarness")),
    Some(parse_path("rstest_bdd_harness_tokio::TokioAttributePolicy")),
    true,
    false
)]
fn generate_test_attrs_honours_harness_precedence(
    #[case] runtime: RuntimeMode,
    #[case] harness_path: Option<syn::Path>,
    #[case] policy_path: Option<syn::Path>,
    #[case] expect_tokio_test: bool,
    #[case] expect_gpui_test: bool,
) {
    let harness = harness_path.as_ref();
    let policy = policy_path.as_ref();
    let tokens = generate_test_attrs(&[], runtime, harness, policy, true);
    let output = tokens.to_string();

    assert!(
        output.contains("rstest :: rstest"),
        "should contain rstest::rstest: {output}"
    );
    assert_eq!(
        output.contains("tokio :: test"),
        expect_tokio_test,
        "tokio::test presence mismatch for runtime={runtime:?}, harness={harness_path:?}, policy={policy_path:?}: {output}"
    );
    assert_eq!(
        output.contains("gpui :: test"),
        expect_gpui_test,
        "gpui::test presence mismatch for runtime={runtime:?}, harness={harness_path:?}, policy={policy_path:?}: {output}"
    );
}
