//! Tests for `&str` reference handling in argument extraction.

use rstest::rstest;
use std::collections::HashSet;
use syn::parse_quote;

#[path = "../src/codegen/wrapper/args/mod.rs"]
#[expect(dead_code, reason = "test reuses only selected helpers")]
mod args_impl;

use args_impl::{ExtractedArgs, extract_args};

#[expect(dead_code, reason = "test uses only selected helpers from support")]
mod support;
use support::{fixture_count, ordered_parameter_names, step_arg_count};

/// Helper for invoking `extract_args` with placeholder names.
fn test_extract_args_scenario(
    func_def: syn::ItemFn,
    placeholders: Vec<&str>,
) -> syn::Result<ExtractedArgs> {
    let mut func = func_def;
    let mut placeholder_set: HashSet<String> = placeholders.into_iter().map(String::from).collect();
    extract_args(&mut func, &mut placeholder_set)
}

#[rstest]
#[case(parse_quote! { fn step(tag: &str) {} }, "&str")]
#[case(parse_quote! { fn step(tag: &'a str) {} }, "&'a str")]
#[case(parse_quote! { fn step(tag: &'static str) {} }, "&'static str")]
fn str_reference_variants_are_classified_as_step_arguments(
    #[case] func: syn::ItemFn,
    #[case] description: &str,
) {
    let args = test_extract_args_scenario(func, vec!["tag"])
        .unwrap_or_else(|e| panic!("failed to extract args for {description}: {e}"));
    assert_eq!(
        step_arg_count(&args),
        1,
        "{description}: unexpected step_arg_count"
    );
    assert_eq!(
        fixture_count(&args),
        0,
        "{description}: unexpected fixture_count"
    );
    assert_eq!(
        ordered_parameter_names(&args),
        ["tag"],
        "{description}: unexpected parameter names"
    );
}

#[rstest]
fn mixed_str_reference_and_parsed_types() {
    let func = parse_quote! { fn step(tag: &str, count: u32, name: String) {} };
    #[expect(clippy::expect_used, reason = "test asserts valid extraction")]
    let args =
        test_extract_args_scenario(func, vec!["tag", "count", "name"]).expect("extraction failed");
    assert_eq!(step_arg_count(&args), 3);
    assert_eq!(fixture_count(&args), 0);
    assert_eq!(ordered_parameter_names(&args), ["tag", "count", "name"]);
}
