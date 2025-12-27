//! Tests for argument preparation helpers.

use super::*;
use crate::codegen::wrapper::args::Arg;
use quote::{format_ident, quote};
use rstest::rstest;
use syn::parse_quote;

fn sample_meta<'a>(pattern: &'a syn::LitStr, ident: &'a syn::Ident) -> StepMeta<'a> {
    StepMeta { pattern, ident }
}

/// Generate step parse code for a single argument with the given type and optional hint.
///
/// This helper encapsulates the common setup for testing `gen_step_parses`:
/// pattern creation, meta creation, argument/capture construction, and
/// token extraction. Returns the generated code as a string for assertions.
fn generate_step_parse_for_single_arg(ty: syn::Type) -> String {
    generate_step_parse_with_hint(ty, None)
}

/// Generate step parse code for a single argument with the given type and hint.
fn generate_step_parse_with_hint(ty: syn::Type, hint: Option<String>) -> String {
    let pattern: syn::LitStr = parse_quote!("test {name}");
    let ident: syn::Ident = parse_quote!(test_step);
    let meta = sample_meta(&pattern, &ident);

    let arg = Arg::Step {
        pat: parse_quote!(name),
        ty,
    };
    let args = vec![&arg];
    let captures = vec![quote! { captures.get(0).map(|m| m.as_str()) }];
    let hints = vec![hint];

    let tokens = gen_step_parses(&args, &captures, &hints, meta);

    #[expect(
        clippy::expect_used,
        reason = "test helper asserts single token output"
    )]
    let token = tokens.first().expect("expected single token stream");
    token.to_string()
}

fn build_arguments() -> Vec<Arg> {
    vec![
        Arg::Fixture {
            pat: parse_quote!(db),
            name: parse_quote!(db),
            ty: parse_quote!(String),
        },
        Arg::Step {
            pat: parse_quote!(count),
            ty: parse_quote!(usize),
        },
        Arg::DataTable {
            pat: parse_quote!(table),
            ty: parse_quote!(Vec<Vec<String>>),
        },
        Arg::DocString {
            pat: parse_quote!(doc),
        },
    ]
}

#[test]
fn prepare_argument_processing_handles_all_argument_types() {
    let args = build_arguments();
    let pattern: syn::LitStr = parse_quote!("^pattern$");
    let ident: syn::Ident = parse_quote!(demo_step);
    let meta = sample_meta(&pattern, &ident);

    let ctx_ident = format_ident!("__rstest_bdd_ctx");
    let placeholder_names = vec![syn::LitStr::new("count", proc_macro2::Span::call_site())];
    let placeholder_hints: Vec<Option<String>> = vec![None];
    let key_ident = format_ident!("__rstest_bdd_table_key_test");
    let cache_ident = format_ident!("__RSTEST_BDD_TABLE_CACHE_TEST");
    let prepared = prepare_argument_processing(
        &args,
        meta,
        &ctx_ident,
        &placeholder_names,
        &placeholder_hints,
        Some((&key_ident, &cache_ident)),
    );

    assert_eq!(prepared.declares.len(), 1);
    let [fixture_stmt] = prepared.declares.as_slice() else {
        panic!("expected single fixture declaration");
    };
    let fixture_code = fixture_stmt.to_string();
    assert!(fixture_code.contains("__rstest_bdd_ctx"));
    assert!(fixture_code.contains("clone"));
    assert!(fixture_code.contains("MissingFixture"));

    assert_eq!(prepared.step_arg_parses.len(), 1);
    let [parse_stmt] = prepared.step_arg_parses.as_slice() else {
        panic!("expected single step argument parser");
    };
    let parse_code = parse_stmt.to_string();
    assert!(parse_code.contains("captures"));
    assert!(parse_code.contains("parse"));

    let Some(datatable_code) = prepared.datatable_decl else {
        panic!("expected datatable declaration");
    };
    assert!(datatable_code.to_string().contains("iter"));

    let Some(docstring_code) = prepared.docstring_decl else {
        panic!("expected docstring declaration");
    };
    assert!(docstring_code.to_string().contains("to_owned"));
}

#[test]
fn collect_ordered_arguments_preserves_call_order() {
    let args = build_arguments();
    let names: Vec<String> = collect_ordered_arguments(&args)
        .into_iter()
        .map(std::string::ToString::to_string)
        .collect();

    assert_eq!(names, ["db", "count", "table", "doc"]);
}

#[test]
fn gen_fixture_decls_handles_reference_types() {
    let fixtures = [
        Arg::Fixture {
            pat: parse_quote!(owned_fixture),
            name: parse_quote!(owned_fixture),
            ty: parse_quote!(String),
        },
        Arg::Fixture {
            pat: parse_quote!(str_fixture),
            name: parse_quote!(str_fixture),
            ty: parse_quote!(&'static str),
        },
        Arg::Fixture {
            pat: parse_quote!(bytes_fixture),
            name: parse_quote!(bytes_fixture),
            ty: parse_quote!(&'static [u8]),
        },
        Arg::Fixture {
            pat: parse_quote!(mut_fixture),
            name: parse_quote!(mut_fixture),
            ty: parse_quote!(&'static mut u32),
        },
        Arg::Fixture {
            pat: parse_quote!(cell_fixture),
            name: parse_quote!(cell_fixture),
            ty: parse_quote!(&std::cell::RefCell<u32>),
        },
    ];
    let ident: syn::Ident = parse_quote!(step_fn);
    let ctx_ident = format_ident!("__rstest_bdd_ctx");
    let fixture_refs: Vec<_> = fixtures.iter().collect();
    let tokens = gen_fixture_decls(&fixture_refs, &ident, &ctx_ident);
    let [owned, str_ref, bytes_ref, mut_ref, cell_ref] = tokens.as_slice() else {
        panic!("expected five fixture declarations");
    };

    let owned_code = owned.to_string();
    assert!(owned_code.contains("clone"));

    let str_code = str_ref.to_string();
    assert!(str_code.contains("value"));
    assert!(!str_code.contains("clone"));

    let bytes_code = bytes_ref.to_string();
    assert!(bytes_code.contains("value"));
    assert!(!bytes_code.contains("clone"));

    let mut_code = mut_ref.to_string();
    assert!(mut_code.contains("borrow_mut"));
    assert!(mut_code.contains("value_mut"));

    let cell_code = cell_ref.to_string();
    assert!(!cell_code.contains("cloned"));
    assert!(!cell_code.contains("copied"));
}

#[test]
fn step_error_tokens_embed_variant_and_message() {
    let variant: syn::Ident = parse_quote!(ExecutionError);
    let pattern: syn::LitStr = parse_quote!("pattern");
    let ident: syn::Ident = parse_quote!(step_fn);
    let message = quote! { "failure".to_string() };

    let tokens = step_error_tokens(&variant, &pattern, &ident, &message).to_string();

    assert!(tokens.contains("StepError :: ExecutionError"));
    assert!(tokens.contains("pattern :"));
    assert!(tokens.contains("function :"));
    assert!(tokens.contains("message :"));
    assert!(tokens.contains(r#""failure""#));
}

#[rstest]
#[case(parse_quote!(&str), "&str")]
#[case(parse_quote!(&'a str), "&'a str")]
#[case(parse_quote!(&'static str), "&'static str")]
fn is_str_reference_detects_borrowed_str(#[case] ty: syn::Type, #[case] description: &str) {
    assert!(is_str_reference(&ty), "{description} should be detected");
}

#[rstest]
#[case(parse_quote!(String), "String should not be detected as &str")]
#[case(parse_quote!(&String), "&String should not be detected as &str")]
#[case(parse_quote!(&mut str), "&mut str should not be supported")]
#[case(parse_quote!(&u8), "&u8 should not be detected as &str")]
#[case(parse_quote!(&[u8]), "&[u8] should not be detected as &str")]
fn is_str_reference_rejects_non_str_references(#[case] ty: syn::Type, #[case] reason: &str) {
    assert!(!is_str_reference(&ty), "{reason}");
}

#[test]
fn gen_step_parses_uses_direct_assignment_for_str_reference() {
    let code = generate_step_parse_for_single_arg(parse_quote!(&str));

    assert!(
        !code.contains("parse"),
        "&str should not use parse(): {code}"
    );
    // Use whitespace-normalised comparison to avoid fragility from token stream formatting
    let normalised = code.split_whitespace().collect::<Vec<_>>().join(" ");
    assert!(
        normalised.contains("__raw0 : & str"),
        "should have typed raw variable: {code}"
    );
}

#[test]
fn gen_step_parses_uses_parse_for_owned_string() {
    let code = generate_step_parse_for_single_arg(parse_quote!(String));

    assert!(code.contains("parse"), "String should use parse(): {code}");
}

#[test]
fn gen_step_parses_handles_mixed_str_and_parsed_types() {
    let pattern: syn::LitStr = parse_quote!("test {tag} {count}");
    let ident: syn::Ident = parse_quote!(test_step);
    let meta = sample_meta(&pattern, &ident);

    let str_arg = Arg::Step {
        pat: parse_quote!(tag),
        ty: parse_quote!(&str),
    };
    let usize_arg = Arg::Step {
        pat: parse_quote!(count),
        ty: parse_quote!(usize),
    };
    let args = vec![&str_arg, &usize_arg];
    let captures = vec![
        quote! { captures.get(0).map(|m| m.as_str()) },
        quote! { captures.get(1).map(|m| m.as_str()) },
    ];

    let hints: Vec<Option<String>> = vec![None, None];
    let tokens = gen_step_parses(&args, &captures, &hints, meta);

    assert_eq!(tokens.len(), 2, "expected two token streams");
    #[expect(clippy::indexing_slicing, reason = "length verified above")]
    let str_code = tokens[0].to_string();
    assert!(
        !str_code.contains("parse"),
        "&str should not use parse(): {str_code}"
    );

    #[expect(clippy::indexing_slicing, reason = "length verified above")]
    let usize_code = tokens[1].to_string();
    assert!(
        usize_code.contains("parse"),
        "usize should use parse(): {usize_code}"
    );
}

#[rstest]
#[case(parse_quote!(&str), false, "&str with :string hint should not use parse()")]
#[case(parse_quote!(String), true, "String with :string hint should use parse()")]
fn gen_step_parses_strips_quotes_for_string_hint(
    #[case] ty: syn::Type,
    #[case] should_parse: bool,
    #[case] parse_description: &str,
) {
    let code = generate_step_parse_with_hint(ty, Some("string".to_string()));

    // Should contain quote stripping code
    assert!(
        code.contains("__raw0 . len () - 1"),
        "should strip quotes: {code}"
    );

    // Conditionally assert parse() presence based on type
    if should_parse {
        assert!(code.contains("parse"), "{parse_description}: {code}");
    } else {
        assert!(!code.contains("parse"), "{parse_description}: {code}");
    }
}

#[test]
fn gen_step_parses_no_quote_strip_without_string_hint() {
    let code = generate_step_parse_for_single_arg(parse_quote!(String));

    // Should NOT contain quote stripping code
    assert!(
        !code.contains("len () - 1"),
        "should not strip quotes without hint: {code}"
    );
}

#[test]
fn gen_step_parses_applies_hints_only_to_matching_arguments() {
    let pattern: syn::LitStr = parse_quote!("test {name} {count}");
    let ident: syn::Ident = parse_quote!(test_step);
    let meta = sample_meta(&pattern, &ident);

    let name_arg = Arg::Step {
        pat: parse_quote!(name),
        ty: parse_quote!(&str),
    };
    let count_arg = Arg::Step {
        pat: parse_quote!(count),
        ty: parse_quote!(usize),
    };
    let args = vec![&name_arg, &count_arg];
    let captures = vec![
        quote! { captures.get(0).map(|m| m.as_str()) },
        quote! { captures.get(1).map(|m| m.as_str()) },
    ];

    // Only first argument has :string hint
    let hints: Vec<Option<String>> = vec![Some("string".to_string()), None];
    let tokens = gen_step_parses(&args, &captures, &hints, meta);

    assert_eq!(tokens.len(), 2, "expected two token streams");

    // First argument (with :string hint) should have quote stripping
    #[expect(clippy::indexing_slicing, reason = "length verified above")]
    let name_code = tokens[0].to_string();
    assert!(
        name_code.contains("__raw0 . len () - 1"),
        "first arg with :string hint should strip quotes: {name_code}"
    );
    assert!(
        !name_code.contains("parse"),
        "&str should not use parse(): {name_code}"
    );

    // Second argument (without hint) should NOT have quote stripping
    #[expect(clippy::indexing_slicing, reason = "length verified above")]
    let count_code = tokens[1].to_string();
    assert!(
        !count_code.contains("len () - 1"),
        "second arg without :string hint should not strip quotes: {count_code}"
    );
    assert!(
        count_code.contains("parse"),
        "usize should use parse(): {count_code}"
    );
}

#[test]
fn gen_step_parses_string_hint_includes_parse_error_message() {
    // When :string hint is used with an owned type, the error message should still
    // reference the original type and pattern for debugging purposes
    let code = generate_step_parse_with_hint(parse_quote!(String), Some("string".to_string()));

    // Should contain error message with type information
    assert!(
        code.contains("failed to parse argument"),
        "should include parse error message: {code}"
    );
    assert!(
        code.contains("stringify ! (name)"),
        "error should reference argument name: {code}"
    );
    assert!(
        code.contains("stringify ! (String)"),
        "error should reference type: {code}"
    );
}

#[rstest]
#[case("u32", "u32 hint should use standard parse path")]
#[case("i64", "i64 hint should use standard parse path")]
#[case("f64", "f64 hint should use standard parse path")]
#[case("unknown", "unknown hint should use standard parse path")]
fn gen_step_parses_non_string_hints_use_standard_parse_path(
    #[case] hint: &str,
    #[case] description: &str,
) {
    let code = generate_step_parse_with_hint(parse_quote!(i32), Some(hint.to_string()));

    // Non-:string hints should NOT strip quotes
    assert!(
        !code.contains("len () - 1"),
        "{description} - should not strip quotes: {code}"
    );
    // Should use standard parse path
    assert!(
        code.contains("parse"),
        "{description} - should use parse(): {code}"
    );
}
