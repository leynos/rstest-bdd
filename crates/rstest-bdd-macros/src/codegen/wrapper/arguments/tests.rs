//! Tests for argument preparation helpers.

use super::*;
use crate::codegen::wrapper::args::Arg;
use quote::{format_ident, quote};
use syn::parse_quote;

fn sample_meta<'a>(pattern: &'a syn::LitStr, ident: &'a syn::Ident) -> StepMeta<'a> {
    StepMeta { pattern, ident }
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
    let prepared = prepare_argument_processing(&args, meta, &ctx_ident, &placeholder_names);

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
