//! Wrapper body assembly for wrapper emission.
//!
//! This module combines prepared argument handling, error reporting, and the
//! call expression into a single wrapper body token stream. It keeps the
//! emission entry point focused on orchestration while centralising the logic
//! that shapes the wrapper's structure.

use super::super::args::ExtractedArgs;
use super::super::arguments::{
    PreparedArgs, StepMeta, collect_ordered_arguments, prepare_argument_processing,
};
use super::call_expr::generate_call_expression;
use super::datatable_cache::{DatatableCacheComponents, generate_datatable_cache_definitions};
use super::errors::{WrapperErrors, prepare_wrapper_errors};
use super::identifiers::generate_wrapper_signature;
use crate::return_classifier::ReturnKind;
use proc_macro2::TokenStream as TokenStream2;
use quote::{format_ident, quote};

const WRAPPER_EXPECT_REASON: &str = "rstest-bdd step wrapper pattern requires these patterns \
for parameter extraction, Result normalization, and closure-based error handling";

/// Prepared wrapper inputs consumed by `assemble_wrapper_function`.
struct WrapperAssembly<'a> {
    meta: StepMeta<'a>,
    prepared: PreparedArgs,
    arg_idents: Vec<syn::Ident>,
    capture_count: usize,
    return_kind: ReturnKind,
}

/// Identifiers used during wrapper generation.
#[derive(Copy, Clone)]
struct WrapperIdentifiers<'a> {
    wrapper: &'a proc_macro2::Ident,
    pattern: &'a proc_macro2::Ident,
    ctx: &'a proc_macro2::Ident,
    text: &'a proc_macro2::Ident,
}

/// Generate the expect attribute for suppressing known Clippy lints in wrapper functions.
fn generate_expect_attribute() -> TokenStream2 {
    quote! {
        #[expect(
            clippy::shadow_reuse,
            clippy::unnecessary_wraps,
            clippy::str_to_string,
            clippy::redundant_closure_for_method_calls,
            clippy::needless_pass_by_value,
            clippy::redundant_closure,
            reason = #WRAPPER_EXPECT_REASON
        )]
    }
}

/// Rendering context for wrapper function generation.
struct WrapperRenderContext<'a> {
    errors: WrapperErrors,
    capture_count: usize,
    call_expr: &'a TokenStream2,
}

/// Render the wrapper function tokens from prepared inputs.
fn render_wrapper_function(
    identifiers: WrapperIdentifiers<'_>,
    prepared: PreparedArgs,
    context: WrapperRenderContext<'_>,
) -> TokenStream2 {
    let WrapperIdentifiers {
        wrapper: wrapper_ident,
        pattern: pattern_ident,
        ctx: ctx_ident,
        text: text_ident,
    } = identifiers;
    let PreparedArgs {
        declares,
        step_arg_parses,
        step_struct_decl,
        datatable_decl,
        docstring_decl,
    } = prepared;
    let WrapperRenderContext {
        errors,
        capture_count,
        call_expr,
    } = context;
    let WrapperErrors {
        placeholder: placeholder_err,
        panic: panic_err,
        execution: exec_err,
        capture_mismatch: capture_mismatch_err,
    } = errors;
    let expected = capture_count;
    let path = crate::codegen::rstest_bdd_path();
    let expect_attr = generate_expect_attribute();
    quote! {
        #expect_attr
        fn #wrapper_ident(
            #ctx_ident: &mut #path::StepContext<'_>,
            #text_ident: &str,
            docstring: Option<&str>,
            table: Option<&[&[&str]]>,
        ) -> Result<#path::StepExecution, #path::StepError> {
            use std::panic::{catch_unwind, AssertUnwindSafe};
            let captures = #path::extract_placeholders(&#pattern_ident, #text_ident.into())
                .map_err(|e| #placeholder_err)?;
            let expected: usize = #expected;
            if captures.len() != expected {
                return Err(#capture_mismatch_err);
            }
            #(#declares)*
            #(#step_arg_parses)*
            #step_struct_decl
            #datatable_decl
            #docstring_decl
            match catch_unwind(AssertUnwindSafe(|| { #call_expr })) {
                Ok(res) => res
                    .map(|value| #path::StepExecution::from_value(value))
                    .map_err(|message| #exec_err),
                Err(payload) => match payload.downcast::<#path::SkipRequest>() {
                    Ok(skip) => Ok(#path::StepExecution::skipped(skip.into_message())),
                    Err(payload) => {
                        let message = #path::panic_message(payload.as_ref());
                        Err(#panic_err)
                    }
                },
            }
        }
    }
}

/// Assemble the final wrapper function using prepared components.
fn assemble_wrapper_function(
    identifiers: WrapperIdentifiers<'_>,
    assembly: WrapperAssembly<'_>,
) -> TokenStream2 {
    let WrapperAssembly {
        meta,
        prepared,
        arg_idents,
        capture_count,
        return_kind,
    } = assembly;
    let WrapperIdentifiers {
        text: text_ident, ..
    } = identifiers;
    let errors = prepare_wrapper_errors(meta, text_ident);
    let StepMeta { ident, .. } = meta;
    let call_expr = generate_call_expression(return_kind, ident, &arg_idents);
    render_wrapper_function(
        identifiers,
        prepared,
        WrapperRenderContext {
            errors,
            capture_count,
            call_expr: &call_expr,
        },
    )
}

/// Generate the compile-time assertion for step struct field count.
fn generate_struct_assertion(args: &ExtractedArgs, capture_count: usize) -> Option<TokenStream2> {
    args.step_struct().map(|arg| {
        let ty = arg.ty;
        let path = crate::codegen::rstest_bdd_path();
        quote! {
            const _: [(); <#ty as #path::step_args::StepArgs>::FIELD_COUNT] = [(); #capture_count];
        }
    })
}

/// Generate datatable cache components and extract identifier references.
fn process_datatable_cache(
    args: &ExtractedArgs,
    wrapper_ident: &proc_macro2::Ident,
) -> (
    TokenStream2,
    Option<(proc_macro2::Ident, proc_macro2::Ident)>,
) {
    let cache_components =
        generate_datatable_cache_definitions(args.datatable().is_some(), wrapper_ident);
    match cache_components {
        DatatableCacheComponents::None => (proc_macro2::TokenStream::new(), None),
        DatatableCacheComponents::Some {
            tokens,
            key_ident,
            cache_ident,
        } => (tokens, Some((key_ident, cache_ident))),
    }
}

/// Generate the wrapper function body and pattern constant.
pub(super) fn generate_wrapper_body(
    config: &super::WrapperConfig<'_>,
    wrapper_ident: &proc_macro2::Ident,
    pattern_ident: &proc_macro2::Ident,
) -> TokenStream2 {
    let super::WrapperConfig {
        ident,
        args,
        pattern,
        placeholder_names,
        placeholder_hints,
        capture_count,
        return_kind,
        ..
    } = *config;

    let ctx_ident = format_ident!("__rstest_bdd_ctx");
    let text_ident = format_ident!("__rstest_bdd_text");
    let args_slice = &args.args;
    let step_meta = StepMeta { pattern, ident };
    let struct_assert = generate_struct_assertion(args, capture_count);
    let signature = generate_wrapper_signature(pattern, pattern_ident);
    let (cache_tokens, datatable_idents) = process_datatable_cache(args, wrapper_ident);
    let datatable_idents_refs = datatable_idents.as_ref().map(|(key, cache)| (key, cache));
    let prepared = prepare_argument_processing(
        args_slice,
        step_meta,
        &ctx_ident,
        placeholder_names,
        placeholder_hints,
        datatable_idents_refs,
    );
    let arg_idents = collect_ordered_arguments(args_slice);
    let wrapper_fn = assemble_wrapper_function(
        WrapperIdentifiers {
            wrapper: wrapper_ident,
            pattern: pattern_ident,
            ctx: &ctx_ident,
            text: &text_ident,
        },
        WrapperAssembly {
            meta: step_meta,
            prepared,
            arg_idents,
            capture_count,
            return_kind,
        },
    );

    quote! {
        #struct_assert
        #cache_tokens
        #signature
        #wrapper_fn
    }
}

#[cfg(test)]
mod tests {
    //! Tests for wrapper lint suppression emission.

    use super::{
        PreparedArgs, StepMeta, WRAPPER_EXPECT_REASON, WrapperAssembly, WrapperIdentifiers,
        assemble_wrapper_function,
    };
    use crate::return_classifier::ReturnKind;
    use proc_macro2::Span;
    use quote::format_ident;
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

    /// Parse and validate the expect attribute from a wrapper function.
    /// Returns (`lint_names`, `reason`, `has_unexpected_meta`).
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
                    reason = Some(
                        extract_reason_from_meta(name_value)
                            .expect("expected reason value to be a string literal"),
                    );
                }
                _ => {
                    unexpected_meta = true;
                }
            }
        }

        (lint_names, reason, unexpected_meta)
    }

    #[test]
    #[expect(
        clippy::expect_used,
        reason = "test validates emitted wrapper attributes"
    )]
    fn wrapper_emits_expect_attribute_for_clippy_lints() {
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
                prepared: PreparedArgs {
                    declares: Vec::new(),
                    step_arg_parses: Vec::new(),
                    step_struct_decl: None,
                    datatable_decl: None,
                    docstring_decl: None,
                },
                arg_idents: Vec::new(),
                capture_count: 0,
                return_kind: ReturnKind::Unit,
            },
        );

        let wrapper_fn: syn::ItemFn = syn::parse2(tokens).expect("wrapper should parse");
        let (lint_names, reason, unexpected_meta) = parse_expect_attribute(&wrapper_fn);

        let expected: HashSet<String> = [
            "clippy::shadow_reuse",
            "clippy::unnecessary_wraps",
            "clippy::str_to_string",
            "clippy::redundant_closure_for_method_calls",
            "clippy::needless_pass_by_value",
            "clippy::redundant_closure",
        ]
        .into_iter()
        .map(String::from)
        .collect();

        assert_eq!(lint_names, expected, "expect attribute lint list mismatch");
        assert!(
            !unexpected_meta,
            "unexpected meta entry in expect attribute"
        );
        assert_eq!(
            reason.as_deref(),
            Some(WRAPPER_EXPECT_REASON),
            "expect attribute reason mismatch",
        );
    }
}
