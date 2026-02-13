//! Unit tests for `scenarios!` macro argument parsing.

use super::{FixtureSpec, RuntimeMode, ScenariosArgs, TestAttributeHint};
use quote::quote;
use syn::parse_quote;

fn try_parse_scenarios_args(tokens: proc_macro2::TokenStream) -> syn::Result<ScenariosArgs> {
    syn::parse2(tokens)
}

#[expect(clippy::expect_used, reason = "test helper with descriptive failures")]
fn parse_scenarios_args(tokens: proc_macro2::TokenStream) -> ScenariosArgs {
    try_parse_scenarios_args(tokens).expect("scenarios args should parse")
}

fn parse_fixture_spec(tokens: proc_macro2::TokenStream) -> syn::Result<FixtureSpec> {
    syn::parse2(tokens)
}

fn type_to_string(ty: &syn::Type) -> String {
    quote!(#ty).to_string()
}

/// Assert that parsing fails and the error message contains the expected keyword.
fn assert_parse_error_contains(result: syn::Result<ScenariosArgs>, expected_keyword: &str) {
    match result {
        Ok(_) => panic!("parsing should fail"),
        Err(err) => {
            let msg = err.to_string();
            assert!(
                msg.contains(expected_keyword),
                "error message should contain '{expected_keyword}': {msg}"
            );
        }
    }
}

/// Assert that fixture spec parsing fails and the error exists.
fn assert_fixture_parse_fails(tokens: proc_macro2::TokenStream) {
    assert!(parse_fixture_spec(tokens).is_err(), "parsing should fail");
}

/// Assert the tag filter matches the expected value.
#[expect(clippy::expect_used, reason = "test helper with descriptive failures")]
fn assert_tag_filter_eq(args: &ScenariosArgs, expected: &str) {
    assert_eq!(
        args.tag_filter
            .as_ref()
            .expect("tag_filter should be set")
            .value(),
        expected
    );
}

#[test]
#[expect(clippy::expect_used, reason = "test with descriptive failures")]
fn fixture_spec_parses_simple_type() {
    let spec: FixtureSpec =
        parse_fixture_spec(parse_quote!(world: TestWorld)).expect("fixture spec should parse");
    assert_eq!(spec.name.to_string(), "world");
    assert!(type_to_string(&spec.ty).contains("TestWorld"));
}

#[test]
#[expect(clippy::expect_used, reason = "test with descriptive failures")]
fn fixture_spec_parses_generic_type() {
    let spec: FixtureSpec = parse_fixture_spec(parse_quote!(counter: RefCell<CounterWorld>))
        .expect("fixture spec should parse");
    assert_eq!(spec.name.to_string(), "counter");
    let ty_str = type_to_string(&spec.ty);
    assert!(ty_str.contains("RefCell"));
    assert!(ty_str.contains("CounterWorld"));
}

#[test]
#[expect(clippy::expect_used, reason = "test with descriptive failures")]
fn fixture_spec_parses_path_type() {
    let spec: FixtureSpec = parse_fixture_spec(parse_quote!(db: std::sync::Arc<Database>))
        .expect("fixture spec should parse");
    assert_eq!(spec.name.to_string(), "db");
}

#[test]
fn fixture_spec_rejects_missing_colon() {
    assert_fixture_parse_fails(parse_quote!(world TestWorld));
}

#[test]
fn fixture_spec_rejects_missing_type() {
    assert_fixture_parse_fails(parse_quote!(world:));
}

#[test]
fn scenarios_args_parses_positional_dir() {
    let args: ScenariosArgs = parse_scenarios_args(parse_quote!("tests/features"));
    assert_eq!(args.dir.value(), "tests/features");
    assert!(args.tag_filter.is_none());
    assert!(args.fixtures.is_empty());
}

#[test]
fn scenarios_args_parses_named_dir() {
    let args: ScenariosArgs = parse_scenarios_args(parse_quote!(dir = "tests/features"));
    assert_eq!(args.dir.value(), "tests/features");
}

#[test]
fn scenarios_args_parses_named_path() {
    let args: ScenariosArgs = parse_scenarios_args(parse_quote!(path = "tests/features"));
    assert_eq!(args.dir.value(), "tests/features");
}

#[test]
fn scenarios_args_parses_with_tags() {
    let args: ScenariosArgs = parse_scenarios_args(parse_quote!("tests/features", tags = "@fast"));
    assert_eq!(args.dir.value(), "tests/features");
    assert_tag_filter_eq(&args, "@fast");
}

#[test]
#[expect(clippy::expect_used, reason = "test with descriptive failures")]
fn scenarios_args_parses_single_fixture() {
    let args: ScenariosArgs =
        parse_scenarios_args(parse_quote!("tests/features", fixtures = [world: TestWorld]));
    assert_eq!(args.fixtures.len(), 1);
    assert_eq!(
        args.fixtures
            .first()
            .expect("first fixture")
            .name
            .to_string(),
        "world"
    );
}

#[test]
#[expect(clippy::expect_used, reason = "test with descriptive failures")]
fn scenarios_args_parses_multiple_fixtures() {
    let args: ScenariosArgs = parse_scenarios_args(parse_quote!(
        "tests/features",
        fixtures = [world: TestWorld, db: Database]
    ));
    assert_eq!(args.fixtures.len(), 2);
    assert_eq!(
        args.fixtures
            .first()
            .expect("first fixture")
            .name
            .to_string(),
        "world"
    );
    assert_eq!(
        args.fixtures
            .get(1)
            .expect("second fixture")
            .name
            .to_string(),
        "db"
    );
}

#[test]
fn scenarios_args_parses_all_arguments() {
    let args: ScenariosArgs = parse_scenarios_args(parse_quote!(
        "tests/features",
        tags = "@smoke",
        fixtures = [world: TestWorld]
    ));
    assert_eq!(args.dir.value(), "tests/features");
    assert_tag_filter_eq(&args, "@smoke");
    assert_eq!(args.fixtures.len(), 1);
}

#[test]
fn scenarios_args_allows_arguments_in_any_order() {
    let args: ScenariosArgs = parse_scenarios_args(parse_quote!(
        fixtures = [world: TestWorld],
        tags = "@smoke",
        dir = "tests/features"
    ));
    assert_eq!(args.dir.value(), "tests/features");
    assert_tag_filter_eq(&args, "@smoke");
    assert_eq!(args.fixtures.len(), 1);
}

#[test]
fn scenarios_args_rejects_missing_dir() {
    let result = try_parse_scenarios_args(parse_quote!(tags = "@fast"));
    assert_parse_error_contains(result, "dir");
}

#[test]
fn scenarios_args_rejects_duplicate_dir() {
    let result = try_parse_scenarios_args(parse_quote!(dir = "a", path = "b"));
    assert_parse_error_contains(result, "duplicate");
}

#[test]
fn scenarios_args_rejects_duplicate_tags() {
    let result = try_parse_scenarios_args(parse_quote!("tests/features", tags = "@a", tags = "@b"));
    assert_parse_error_contains(result, "duplicate");
}

#[test]
fn scenarios_args_rejects_duplicate_fixtures() {
    let result = try_parse_scenarios_args(parse_quote!(
        "tests/features",
        fixtures = [a: A],
        fixtures = [b: B]
    ));
    assert_parse_error_contains(result, "duplicate");
}

#[test]
fn scenarios_args_rejects_unknown_argument() {
    let result = try_parse_scenarios_args(parse_quote!("tests/features", unknown = "value"));
    assert!(result.is_err());
}

#[test]
fn scenarios_args_parses_empty_fixtures() {
    let args: ScenariosArgs = parse_scenarios_args(parse_quote!("tests/features", fixtures = []));
    assert!(args.fixtures.is_empty());
}

#[test]
fn scenarios_args_parses_fixtures_with_trailing_comma() {
    let args: ScenariosArgs = parse_scenarios_args(parse_quote!(
        "tests/features",
        fixtures = [world: TestWorld,]
    ));
    assert_eq!(args.fixtures.len(), 1);
}

#[test]
fn scenarios_args_defaults_to_sync_runtime() {
    let args: ScenariosArgs = parse_scenarios_args(parse_quote!("tests/features"));
    assert_eq!(args.runtime, RuntimeMode::Sync);
    assert!(!args.runtime.is_async());
}

#[test]
fn scenarios_args_parses_runtime_tokio_current_thread() {
    let args: ScenariosArgs = parse_scenarios_args(parse_quote!(
        "tests/features",
        runtime = "tokio-current-thread"
    ));
    assert_eq!(args.runtime, RuntimeMode::TokioCurrentThread);
    assert!(args.runtime.is_async());
}

#[test]
fn scenarios_args_parses_runtime_with_other_arguments() {
    let args: ScenariosArgs = parse_scenarios_args(parse_quote!(
        "tests/features",
        tags = "@async",
        runtime = "tokio-current-thread",
        fixtures = [world: TestWorld]
    ));
    assert_eq!(args.dir.value(), "tests/features");
    assert_tag_filter_eq(&args, "@async");
    assert_eq!(args.runtime, RuntimeMode::TokioCurrentThread);
    assert_eq!(args.fixtures.len(), 1);
}

#[test]
fn scenarios_args_rejects_unknown_runtime() {
    let result =
        try_parse_scenarios_args(parse_quote!("tests/features", runtime = "unknown-runtime"));
    assert_parse_error_contains(result, "unknown runtime");
}

#[test]
fn scenarios_args_rejects_duplicate_runtime() {
    let result = try_parse_scenarios_args(parse_quote!(
        "tests/features",
        runtime = "tokio-current-thread",
        runtime = "tokio-current-thread"
    ));
    assert_parse_error_contains(result, "duplicate");
}

#[rstest::rstest]
#[case::sync(RuntimeMode::Sync, TestAttributeHint::RstestOnly)]
#[case::tokio(
    RuntimeMode::TokioCurrentThread,
    TestAttributeHint::RstestWithTokioCurrentThread
)]
fn runtime_mode_returns_expected_hint(
    #[case] mode: RuntimeMode,
    #[case] expected: TestAttributeHint,
) {
    assert_eq!(mode.test_attribute_hint(), expected);
}

#[test]
#[expect(clippy::expect_used, reason = "test with descriptive failures")]
fn scenarios_args_parses_harness_argument() {
    let args: ScenariosArgs = parse_scenarios_args(parse_quote!(
        "tests/features",
        harness = rstest_bdd_harness::StdHarness
    ));
    assert_eq!(args.dir.value(), "tests/features");
    let harness = args.harness.expect("harness should be set");
    let harness_str = quote!(#harness).to_string();
    assert!(
        harness_str.contains("StdHarness"),
        "should contain StdHarness: {harness_str}"
    );
    assert!(args.attributes.is_none());
}

#[test]
#[expect(clippy::expect_used, reason = "test with descriptive failures")]
fn scenarios_args_parses_attributes_argument() {
    let args: ScenariosArgs = parse_scenarios_args(parse_quote!(
        "tests/features",
        attributes = rstest_bdd_harness::DefaultAttributePolicy
    ));
    let attr_policy = args.attributes.expect("attributes should be set");
    let attr_str = quote!(#attr_policy).to_string();
    assert!(
        attr_str.contains("DefaultAttributePolicy"),
        "should contain DefaultAttributePolicy: {attr_str}"
    );
    assert!(args.harness.is_none());
}

#[test]
fn scenarios_args_parses_harness_and_attributes_together() {
    let args: ScenariosArgs = parse_scenarios_args(parse_quote!(
        "tests/features",
        harness = my::Harness,
        attributes = my::Policy
    ));
    assert!(args.harness.is_some());
    assert!(args.attributes.is_some());
}

#[test]
fn scenarios_args_parses_harness_with_all_other_arguments() {
    let args: ScenariosArgs = parse_scenarios_args(parse_quote!(
        "tests/features",
        tags = "@smoke",
        runtime = "tokio-current-thread",
        fixtures = [world: TestWorld],
        harness = my::Harness,
        attributes = my::Policy
    ));
    assert_eq!(args.dir.value(), "tests/features");
    assert_tag_filter_eq(&args, "@smoke");
    assert_eq!(args.runtime, RuntimeMode::TokioCurrentThread);
    assert_eq!(args.fixtures.len(), 1);
    assert!(args.harness.is_some());
    assert!(args.attributes.is_some());
}

#[test]
fn scenarios_args_defaults_harness_and_attributes_to_none() {
    let args: ScenariosArgs = parse_scenarios_args(parse_quote!("tests/features"));
    assert!(args.harness.is_none());
    assert!(args.attributes.is_none());
}

#[test]
fn scenarios_args_rejects_duplicate_harness() {
    let result = try_parse_scenarios_args(parse_quote!(
        "tests/features",
        harness = a::H,
        harness = b::H
    ));
    assert_parse_error_contains(result, "duplicate");
}

#[test]
fn scenarios_args_rejects_duplicate_attributes() {
    let result = try_parse_scenarios_args(parse_quote!(
        "tests/features",
        attributes = a::P,
        attributes = b::P
    ));
    assert_parse_error_contains(result, "duplicate");
}
