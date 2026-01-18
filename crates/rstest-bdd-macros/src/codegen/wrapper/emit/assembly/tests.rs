//! Tests for wrapper lint suppression emission.

use super::{
    LINT_NEEDLESS_PASS_BY_VALUE, LINT_REDUNDANT_CLOSURE, LINT_REDUNDANT_CLOSURE_FOR_METHOD_CALLS,
    LINT_SHADOW_REUSE, LINT_STR_TO_STRING, LINT_UNNECESSARY_WRAPS, PreparedArgs, StepMeta,
    WRAPPER_EXPECT_REASON, WrapperAssembly, WrapperIdentifiers, assemble_wrapper_function,
};
use crate::return_classifier::ReturnKind;
use proc_macro2::Span;
use quote::{format_ident, quote};
use rstest::rstest;
use std::collections::HashSet;
use syn::Token;
use syn::punctuated::Punctuated;

fn path_to_string(path: &syn::Path) -> String {
    path.segments
        .iter()
        .map(|segment| segment.ident.to_string())
        .collect::<Vec<_>>()
        .join("::")
}

fn extract_reason_from_meta(name_value: &syn::MetaNameValue) -> Option<String> {
    if let syn::Expr::Lit(syn::ExprLit {
        lit: syn::Lit::Str(lit_str),
        ..
    }) = &name_value.value
    {
        Some(lit_str.value())
    } else {
        None
    }
}

fn expected_lints(lints: &[&str]) -> HashSet<String> {
    lints.iter().map(|lint| (*lint).to_string()).collect()
}

/// Parse and validate the expect attribute from a wrapper function.
/// Returns (`lint_names`, reason, `has_unexpected_meta`).
#[expect(
    clippy::expect_used,
    reason = "test helper asserts wrapper expect attribute presence and shape"
)]
fn parse_expect_attribute(wrapper_fn: &syn::ItemFn) -> (HashSet<String>, Option<String>, bool) {
    let expect_attr = wrapper_fn
        .attrs
        .iter()
        .find(|attr| attr.path().is_ident("expect"))
        .expect("wrapper should include expect attribute");
    let metas: Punctuated<syn::Meta, Token![,]> = expect_attr
        .parse_args_with(Punctuated::parse_terminated)
        .expect("parse expect attribute arguments");

    let mut lint_names = HashSet::new();
    let mut reason = None;
    let mut unexpected_meta = false;
    for meta in metas {
        match meta {
            syn::Meta::Path(path) => {
                lint_names.insert(path_to_string(&path));
            }
            syn::Meta::NameValue(ref name_value) if name_value.path.is_ident("reason") => {
                reason = extract_reason_from_meta(name_value);
                unexpected_meta |= reason.is_none();
            }
            _ => unexpected_meta = true,
        }
    }

    (lint_names, reason, unexpected_meta)
}

#[expect(
    clippy::expect_used,
    reason = "test helper ensures wrapper tokens parse as a function"
)]
fn assemble_wrapper_for_test(
    prepared: PreparedArgs,
    capture_count: usize,
    return_kind: ReturnKind,
) -> syn::ItemFn {
    let wrapper_ident = format_ident!("__rstest_bdd_wrapper_test");
    let pattern_ident = format_ident!("__RSTEST_BDD_PATTERN_TEST");
    let ctx_ident = format_ident!("__rstest_bdd_ctx");
    let text_ident = format_ident!("__rstest_bdd_text");
    let step_ident = format_ident!("step_given");
    let pattern = syn::LitStr::new("a value {x:string}", Span::call_site());

    let tokens = assemble_wrapper_function(
        WrapperIdentifiers {
            wrapper: &wrapper_ident,
            pattern: &pattern_ident,
            ctx: &ctx_ident,
            text: &text_ident,
        },
        WrapperAssembly {
            meta: StepMeta {
                pattern: &pattern,
                ident: &step_ident,
            },
            prepared,
            arg_idents: Vec::new(),
            capture_count,
            return_kind,
        },
    );

    syn::parse2(tokens).expect("wrapper should parse")
}

/// Helper to assert that a wrapper emits the expected Clippy expect attribute.
fn assert_wrapper_expect_lints(
    step_struct_decl: Option<proc_macro2::TokenStream>,
    has_step_arg_quote_strip: bool,
    capture_count: usize,
    return_kind: ReturnKind,
    expected_lint_names: &[&str],
) {
    let prepared = PreparedArgs {
        declares: Vec::new(),
        step_arg_parses: Vec::new(),
        step_struct_decl,
        datatable_decl: None,
        docstring_decl: None,
        expect_lints: Vec::new(),
        has_step_arg_quote_strip,
    };
    let wrapper_fn = assemble_wrapper_for_test(prepared, capture_count, return_kind);
    let (lint_names, reason, unexpected_meta) = parse_expect_attribute(&wrapper_fn);

    let expected = expected_lints(expected_lint_names);

    assert!(
        !unexpected_meta,
        "unexpected meta entry in expect attribute"
    );
    assert_eq!(lint_names, expected, "expect attribute lint list mismatch");
    assert_eq!(reason.as_deref(), Some(WRAPPER_EXPECT_REASON));
}

#[rstest]
#[case::wrapper_emits_expect_attribute_for_clippy_lints(
    None,
    true,
    1,
    ReturnKind::Unit,
    &[
        LINT_SHADOW_REUSE,
        LINT_UNNECESSARY_WRAPS,
        LINT_REDUNDANT_CLOSURE_FOR_METHOD_CALLS,
        LINT_NEEDLESS_PASS_BY_VALUE,
        LINT_REDUNDANT_CLOSURE,
    ],
)]
#[case::wrapper_emits_expect_attribute_for_step_structs(
    Some(quote! {}),
    false,
    1,
    ReturnKind::ResultValue,
    &[
        LINT_STR_TO_STRING,
        LINT_REDUNDANT_CLOSURE_FOR_METHOD_CALLS,
        LINT_NEEDLESS_PASS_BY_VALUE,
        LINT_REDUNDANT_CLOSURE,
    ],
)]
fn wrapper_emits_expect_attribute(
    #[case] step_struct_decl: Option<proc_macro2::TokenStream>,
    #[case] has_step_arg_quote_strip: bool,
    #[case] capture_count: usize,
    #[case] return_kind: ReturnKind,
    #[case] expected_lint_names: &'static [&'static str],
) {
    assert_wrapper_expect_lints(
        step_struct_decl,
        has_step_arg_quote_strip,
        capture_count,
        return_kind,
        expected_lint_names,
    );
}
