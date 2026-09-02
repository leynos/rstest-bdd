//! Unit tests for `#[scenario]` attribute argument parsing.

use quote::quote;
use syn::parse_quote;

use super::ScenarioArgs;

fn parse_scenario_args(tokens: proc_macro2::TokenStream) -> syn::Result<ScenarioArgs> {
    syn::parse2(tokens)
}

fn assert_parse_error_contains(result: syn::Result<ScenarioArgs>, expected_keyword: &str) {
    match result {
        Ok(_) => panic!("parsing should fail"),
        Err(err) => {
            let msg = err.to_string();
            assert!(
                msg.contains(expected_keyword),
                "error should contain '{expected_keyword}': {msg}"
            );
        }
    }
}

#[test]
fn parses_harness_argument() {
    let args = parse_scenario_args(quote!(
        path = "test.feature",
        harness = rstest_bdd_harness::StdHarness
    ))
    .expect("scenario args should parse");
    assert_eq!(args.path.value(), "test.feature");
    let harness = args.harness.expect("harness should be set");
    let harness_str = quote!(#harness).to_string();
    assert!(
        harness_str.contains("StdHarness"),
        "should contain StdHarness: {harness_str}"
    );
}

#[test]
fn parses_attributes_argument() {
    let args = parse_scenario_args(quote!(
        path = "test.feature",
        attributes = rstest_bdd_harness::DefaultAttributePolicy
    ))
    .expect("scenario args should parse");
    let attr_policy = args.attributes.expect("attributes should be set");
    let attr_str = quote!(#attr_policy).to_string();
    assert!(
        attr_str.contains("DefaultAttributePolicy"),
        "should contain DefaultAttributePolicy: {attr_str}"
    );
}

#[test]
fn parses_harness_and_attributes_together() {
    let args = parse_scenario_args(quote!(
        path = "test.feature",
        harness = my::Harness,
        attributes = my::Policy
    ))
    .expect("scenario args should parse");
    assert!(args.harness.is_some());
    assert!(args.attributes.is_some());
}

#[test]
fn parses_harness_with_all_other_arguments() {
    let args = parse_scenario_args(quote!(
        path = "test.feature",
        name = "My scenario",
        tags = "@fast",
        harness = my::Harness,
        attributes = my::Policy
    ))
    .expect("scenario args should parse");
    assert_eq!(args.path.value(), "test.feature");
    assert!(args.selector.is_some());
    assert!(args.tag_filter.is_some());
    assert!(args.harness.is_some());
    assert!(args.attributes.is_some());
}

#[test]
fn defaults_harness_and_attributes_to_none() {
    let args =
        parse_scenario_args(quote!(path = "test.feature")).expect("scenario args should parse");
    assert!(args.harness.is_none());
    assert!(args.attributes.is_none());
}

#[test]
fn parses_libraries_in_declaration_order() {
    let args = parse_scenario_args(quote!(
        path = "test.feature",
        libraries = [accounts, filesystem]
    ))
    .expect("scenario args should parse");
    let libraries = args.libraries.expect("libraries should be set");
    let paths: Vec<_> = libraries
        .iter()
        .map(|path| quote!(#path).to_string())
        .collect();
    assert_eq!(paths, ["accounts", "filesystem"]);
}

#[test]
fn rejects_duplicate_libraries() {
    let result = parse_scenario_args(quote!(
        path = "test.feature",
        libraries = [accounts],
        libraries = [filesystem]
    ));
    assert_parse_error_contains(result, "duplicate");
}

#[test]
fn rejects_repeated_library_paths() {
    let result = parse_scenario_args(quote!(
        path = "test.feature",
        libraries = [accounts, accounts]
    ));
    assert_parse_error_contains(result, "duplicate step library");
}

#[test]
fn rejects_duplicate_harness() {
    let result = parse_scenario_args(quote!(
        path = "test.feature",
        harness = a::H,
        harness = b::H
    ));
    assert_parse_error_contains(result, "duplicate");
}

#[test]
fn rejects_duplicate_attributes() {
    let result = parse_scenario_args(quote!(
        path = "test.feature",
        attributes = a::P,
        attributes = b::P
    ));
    assert_parse_error_contains(result, "duplicate");
}

#[test]
fn rejects_unknown_argument() {
    let result = parse_scenario_args(quote!(path = "test.feature", unknown = "value"));
    assert!(result.is_err());
}

#[test]
fn global_marker_special_case_is_limited_to_the_runtime_crate() {
    let builtin: syn::Path = parse_quote!(rstest_bdd::global);
    let user_library: syn::Path = parse_quote!(steps::global);

    let builtin_tokens = super::library_marker_path(&builtin).to_string();
    let user_tokens = super::library_marker_path(&user_library).to_string();

    assert!(
        builtin_tokens.contains("global :: STEP_LIBRARY"),
        "{builtin_tokens}"
    );
    assert!(
        !builtin_tokens.contains("__RSTEST_BDD_STEP_LIBRARY_global"),
        "{builtin_tokens}"
    );
    assert!(
        user_tokens.contains("__RSTEST_BDD_STEP_LIBRARY_global"),
        "{user_tokens}"
    );
    assert!(
        !user_tokens.contains("global :: STEP_LIBRARY"),
        "{user_tokens}"
    );
}
