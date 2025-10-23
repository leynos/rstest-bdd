//! Tests for argument preparation helpers.

use super::*;
use crate::codegen::wrapper::args::{
    ArgumentCollections, CallArg, DataTableArg, DocStringArg, FixtureArg, StepArg,
};
use quote::{format_ident, quote};
use syn::parse_quote;

fn sample_meta<'a>(pattern: &'a syn::LitStr, ident: &'a syn::Ident) -> StepMeta<'a> {
    StepMeta { pattern, ident }
}

fn build_arguments() -> (Vec<FixtureArg>, Vec<StepArg>, DataTableArg, DocStringArg) {
    let fixtures = vec![FixtureArg {
        pat: parse_quote!(db),
        name: parse_quote!(db),
        ty: parse_quote!(String),
    }];
    let step_args = vec![StepArg {
        pat: parse_quote!(count),
        ty: parse_quote!(usize),
    }];
    let datatable = DataTableArg {
        pat: parse_quote!(table),
        ty: parse_quote!(Vec<Vec<String>>),
    };
    let docstring = DocStringArg {
        pat: parse_quote!(doc),
    };
    (fixtures, step_args, datatable, docstring)
}

#[test]
fn prepare_argument_processing_handles_all_argument_types() {
    let (fixtures, step_args, datatable, docstring) = build_arguments();
    let collections = ArgumentCollections {
        fixtures: &fixtures,
        step_args: &step_args,
        datatable: Some(&datatable),
        docstring: Some(&docstring),
    };
    let pattern: syn::LitStr = parse_quote!("^pattern$");
    let ident: syn::Ident = parse_quote!(demo_step);
    let meta = sample_meta(&pattern, &ident);

    let ctx_ident = format_ident!("__rstest_bdd_ctx");
    let prepared = prepare_argument_processing(&collections, meta, &ctx_ident);

    assert_eq!(prepared.declares.len(), 1);
    let [fixture_stmt] = prepared.declares.as_slice() else {
        panic!("expected single fixture declaration");
    };
    let fixture_code = fixture_stmt.to_string();
    assert!(fixture_code.contains("__rstest_bdd_ctx"));
    assert!(fixture_code.contains("cloned"));
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
    let (fixtures, step_args, datatable, docstring) = build_arguments();
    let collections = ArgumentCollections {
        fixtures: &fixtures,
        step_args: &step_args,
        datatable: Some(&datatable),
        docstring: Some(&docstring),
    };
    let order = [
        CallArg::Fixture(0),
        CallArg::StepArg(0),
        CallArg::DataTable,
        CallArg::DocString,
    ];
    let names: Vec<String> = collect_ordered_arguments(&order, &collections)
        .into_iter()
        .map(std::string::ToString::to_string)
        .collect();

    assert_eq!(names, ["db", "count", "table", "doc"]);
}

#[test]
fn gen_fixture_decls_handles_reference_types() {
    let fixtures = vec![
        FixtureArg {
            pat: parse_quote!(owned_fixture),
            name: parse_quote!(owned_fixture),
            ty: parse_quote!(String),
        },
        FixtureArg {
            pat: parse_quote!(str_fixture),
            name: parse_quote!(str_fixture),
            ty: parse_quote!(&'static str),
        },
        FixtureArg {
            pat: parse_quote!(bytes_fixture),
            name: parse_quote!(bytes_fixture),
            ty: parse_quote!(&'static [u8]),
        },
        FixtureArg {
            pat: parse_quote!(mut_fixture),
            name: parse_quote!(mut_fixture),
            ty: parse_quote!(&'static mut u32),
        },
        FixtureArg {
            pat: parse_quote!(cell_fixture),
            name: parse_quote!(cell_fixture),
            ty: parse_quote!(&std::cell::RefCell<u32>),
        },
    ];
    let ident: syn::Ident = parse_quote!(step_fn);
    let ctx_ident = format_ident!("__rstest_bdd_ctx");
    let tokens = gen_fixture_decls(&fixtures, &ident, &ctx_ident);
    let [owned, str_ref, bytes_ref, mut_ref, cell_ref] = tokens.as_slice() else {
        panic!("expected five fixture declarations");
    };

    let owned_code = owned.to_string();
    assert!(owned_code.contains("cloned"));

    let str_code = str_ref.to_string();
    assert!(str_code.contains("copied"));

    let bytes_code = bytes_ref.to_string();
    assert!(bytes_code.contains("copied"));

    let mut_code = mut_ref.to_string();
    let mut_compact: String = mut_code.chars().filter(|c| !c.is_whitespace()).collect();
    assert!(mut_compact.contains("map(|value|&mut**value)"));

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
