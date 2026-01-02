//! Step-struct argument code generation.
//!
//! Step-struct arguments (from `#[step_args]`) bundle all placeholder captures
//! into a single type implementing `TryFrom<Vec<String>>`. This module emits
//! the capture collection and conversion code, including `:string` hint
//! handling for quoted captures.

use super::step_parse::gen_quote_strip_to_stripped;
use super::{StepMeta, step_error_tokens};
use crate::codegen::wrapper::args::StepStructArg;
use proc_macro2::TokenStream as TokenStream2;
use quote::{format_ident, quote};

/// Placeholder information needed for step struct code generation.
pub(super) struct PlaceholderInfo<'a> {
    pub(super) captures: &'a [TokenStream2],
    pub(super) names: &'a [syn::LitStr],
    pub(super) hints: &'a [Option<String>],
}

/// Context for generating capture initialisers in struct-based step arguments.
///
/// Groups the parameters required by [`generate_capture_initializers`] to reduce
/// the function's argument count.
struct CaptureInitContext<'a> {
    captures: &'a [TokenStream2],
    missing_errs: &'a [TokenStream2],
    hints: &'a [Option<String>],
    values_ident: &'a proc_macro2::Ident,
    meta: StepMeta<'a>,
    struct_pat: &'a syn::Ident,
}

fn generate_missing_capture_errors(
    placeholder_names: &[syn::LitStr],
    pattern: &syn::LitStr,
    ident: &syn::Ident,
    pat: &syn::Ident,
) -> Vec<TokenStream2> {
    placeholder_names
        .iter()
        .map(|name| {
            step_error_tokens(
                &format_ident!("ExecutionError"),
                pattern,
                ident,
                &quote! {
                    format!(
                        "pattern '{}' missing capture for placeholder '{{{}}}' required by '{}'",
                        #pattern,
                        #name,
                        stringify!(#pat),
                    )
                },
            )
        })
        .collect()
}

fn generate_capture_initializers(ctx: &CaptureInitContext<'_>) -> Vec<TokenStream2> {
    let CaptureInitContext {
        captures,
        missing_errs,
        hints,
        values_ident,
        meta,
        struct_pat,
    } = ctx;
    let StepMeta { pattern, ident } = meta;
    let raw_ident = format_ident!("raw");
    captures
        .iter()
        .zip(missing_errs.iter())
        .enumerate()
        .map(|(idx, (capture, missing))| {
            let hint = hints.get(idx).and_then(|h| h.as_deref());
            let needs_quote_strip = rstest_bdd_patterns::requires_quote_stripping(hint);
            if needs_quote_strip {
                let malformed_err = step_error_tokens(
                    &format_ident!("ExecutionError"),
                    pattern,
                    ident,
                    &quote! {
                        format!(
                            "malformed quoted string for '{}' capture {}: expected at least 2 characters, got '{}'",
                            stringify!(#struct_pat),
                            #idx,
                            #raw_ident,
                        )
                    },
                );
                let quote_strip = gen_quote_strip_to_stripped(&raw_ident, &malformed_err);
                quote! {
                    let #raw_ident = #capture.ok_or_else(|| #missing)?;
                    #quote_strip
                    #values_ident.push(stripped.to_string());
                }
            } else {
                quote! {
                    let #raw_ident = #capture.ok_or_else(|| #missing)?;
                    #values_ident.push(#raw_ident.to_string());
                }
            }
        })
        .collect()
}

pub(super) fn gen_step_struct_decl(
    step_struct: Option<super::BoundStepStructArg<'_>>,
    placeholders: &PlaceholderInfo<'_>,
    meta: StepMeta<'_>,
) -> Option<TokenStream2> {
    let PlaceholderInfo {
        captures,
        names,
        hints,
    } = placeholders;
    let capture_count = names.len();
    step_struct.map(|arg| {
        let StepStructArg { pat, ty } = arg.arg;
        let binding = arg.binding;
        let values_ident = format_ident!("__rstest_bdd_struct_values");
        let StepMeta { pattern, ident } = meta;
        let missing_errs = generate_missing_capture_errors(names, pattern, ident, pat);
        let capture_init_ctx = CaptureInitContext {
            captures,
            missing_errs: &missing_errs,
            hints,
            values_ident: &values_ident,
            meta,
            struct_pat: pat,
        };
        let capture_inits = generate_capture_initializers(&capture_init_ctx);
        let convert_err = step_error_tokens(
            &format_ident!("ExecutionError"),
            pattern,
            ident,
            &quote! {
                format!(
                    "failed to populate '{}' from pattern '{}': {}",
                    stringify!(#pat),
                    #pattern,
                    error
                )
            },
        );
        quote! {
            let mut #values_ident = Vec::with_capacity(#capture_count);
            #(#capture_inits)*
            let #binding: #ty = ::std::convert::TryFrom::try_from(#values_ident)
                .map_err(|error| #convert_err)?;
        }
    })
}
