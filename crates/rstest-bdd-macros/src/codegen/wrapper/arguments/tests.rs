//! Tests for argument preparation helpers.

use super::*;
use crate::codegen::wrapper::args::Arg;
use quote::{format_ident, quote};
use rstest::rstest;
use syn::parse_quote;

fn sample_meta<'a>(pattern: &'a syn::LitStr, ident: &'a syn::Ident) -> StepMeta<'a> {
    StepMeta { pattern, ident }
}

/// Generate step parse code for a single argument with the given type.
///
/// This helper encapsulates the common setup for testing `gen_step_parses`:
/// pattern creation, meta creation, argument/capture construction, and
/// token extraction. Returns the generated code as a string for assertions.
fn generate_step_parse_for_single_arg(ty: syn::Type) -> String {
    let pattern: syn::LitStr = parse_quote!("test {name}");
    let ident: syn::Ident = parse_quote!(test_step);
    let meta = sample_meta(&pattern, &ident);

    let arg = Arg::Step {
        pat: parse_quote!(name),
        ty,
    };
    let args = vec![&arg];
    let captures = vec![quote! { captures.get(0).map(|m| m.as_str()) }];

    let tokens = gen_step_parses(&args, &captures, meta);

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
    let key_ident = format_ident!("__rstest_bdd_table_key_test");
    let cache_ident = format_ident!("__RSTEST_BDD_TABLE_CACHE_TEST");
    let prepared = prepare_argument_processing(
        &args,
        meta,
        &ctx_ident,
        &placeholder_names,
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

    let tokens = gen_step_parses(&args, &captures, meta);

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
