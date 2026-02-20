//! Parameterized tests for combined `scenarios!` argument parsing cases.

use super::{
    RuntimeCompatibilityAlias, RuntimeMode, ScenariosArgs, assert_tag_filter_eq,
    parse_scenarios_args, runtime_compatibility_alias,
};
use syn::parse_quote;

#[rstest::rstest]
#[case::scenarios_args_parses_all_arguments(
    (
        parse_quote!(
            "tests/features",
            tags = "@smoke",
            fixtures = [world: TestWorld]
        ),
        "tests/features",
        Some("@smoke"),
        RuntimeMode::Sync,
        None,
        1,
        false,
        false,
    ),
)]
#[case::scenarios_args_allows_arguments_in_any_order(
    (
        parse_quote!(
            fixtures = [world: TestWorld],
            tags = "@smoke",
            dir = "tests/features"
        ),
        "tests/features",
        Some("@smoke"),
        RuntimeMode::Sync,
        None,
        1,
        false,
        false,
    ),
)]
#[case::scenarios_args_parses_runtime_with_other_arguments(
    (
        parse_quote!(
            "tests/features",
            tags = "@async",
            runtime = "tokio-current-thread",
            fixtures = [world: TestWorld]
        ),
        "tests/features",
        Some("@async"),
        RuntimeMode::TokioCurrentThread,
        Some(RuntimeCompatibilityAlias::TokioHarnessAdapter),
        1,
        false,
        false,
    ),
)]
#[case::scenarios_args_parses_harness_with_all_other_arguments(
    (
        parse_quote!(
            "tests/features",
            tags = "@smoke",
            runtime = "tokio-current-thread",
            fixtures = [world: TestWorld],
            harness = my::Harness,
            attributes = my::Policy
        ),
        "tests/features",
        Some("@smoke"),
        RuntimeMode::TokioCurrentThread,
        Some(RuntimeCompatibilityAlias::TokioHarnessAdapter),
        1,
        true,
        true,
    ),
)]
fn scenarios_args_parses_combined_arguments(
    #[case] (
        input,
        expected_dir,
        expected_tag,
        expected_runtime,
        expected_alias,
        expected_fixtures_len,
        has_harness,
        has_attributes,
    ): (
        proc_macro2::TokenStream,
        &'static str,
        Option<&'static str>,
        RuntimeMode,
        Option<RuntimeCompatibilityAlias>,
        usize,
        bool,
        bool,
    ),
) {
    let args: ScenariosArgs = parse_scenarios_args(input);
    assert_eq!(args.dir.value(), expected_dir);
    if let Some(expected_tag) = expected_tag {
        assert_tag_filter_eq(&args, expected_tag);
    } else {
        assert!(args.tag_filter.is_none());
    }
    assert_eq!(args.runtime, expected_runtime);
    assert_eq!(runtime_compatibility_alias(args.runtime), expected_alias);
    assert_eq!(args.fixtures.len(), expected_fixtures_len);
    assert_eq!(args.harness.is_some(), has_harness);
    assert_eq!(args.attributes.is_some(), has_attributes);
}
