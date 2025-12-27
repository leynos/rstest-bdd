//! Single step argument parsing code generation.
//!
//! This module contains the logic for generating code that parses individual
//! step arguments from regex captures, including support for the `:string` type
//! hint which strips surrounding quotes.

use super::super::args::Arg;
use super::{StepMeta, is_str_reference, step_error_tokens};
use proc_macro2::TokenStream as TokenStream2;
use quote::{format_ident, quote};

/// Generate a token stream that validates and strips surrounding quotes.
///
/// The generated code validates the captured string (bound to `raw_ident`) has
/// at least 2 characters (the surrounding quotes) before slicing. On success,
/// it binds `stripped` to the substring excluding the first and last characters.
///
/// This helper is shared between single-argument parsing and struct-based
/// capture initialisation.
pub(super) fn gen_quote_strip_to_stripped(
    raw_ident: &proc_macro2::Ident,
    malformed_err: &TokenStream2,
) -> TokenStream2 {
    quote! {
        if #raw_ident.len() < 2 {
            return Err(#malformed_err);
        }
        let stripped = &#raw_ident[1..#raw_ident.len() - 1];
    }
}

/// Generate a token stream that strips surrounding quotes from a string slice.
///
/// The generated code validates the captured string has at least 2 characters
/// (the surrounding quotes) before slicing, then reassigns `raw_ident` to the
/// substring excluding the first and last characters.
///
/// This variant is used for single step argument parsing where the raw binding
/// needs to be shadowed with the stripped value.
fn gen_quote_strip(raw_ident: &proc_macro2::Ident, malformed_err: &TokenStream2) -> TokenStream2 {
    quote! {
        if #raw_ident.len() < 2 {
            return Err(#malformed_err);
        }
        let #raw_ident: &str = &#raw_ident[1..#raw_ident.len() - 1];
    }
}

/// Generate an error token stream for failed argument parsing.
///
/// This helper constructs the `StepError::ExecutionError` variant used when
/// `.parse()` fails to convert a captured string to the expected type.
fn gen_parse_err(
    pattern: &syn::LitStr,
    ident: &syn::Ident,
    pat: &syn::Ident,
    ty: &syn::Type,
    raw_ident: &proc_macro2::Ident,
) -> TokenStream2 {
    step_error_tokens(
        &format_ident!("ExecutionError"),
        pattern,
        ident,
        &quote! {
            format!(
                "failed to parse argument '{}' of type '{}' from pattern '{}' with captured value: '{:?}'",
                stringify!(#pat),
                stringify!(#ty),
                #pattern,
                #raw_ident,
            )
        },
    )
}

/// Generate code to bind a `&str` reference argument from a regex capture.
///
/// Optionally strips surrounding quotes if the hint requires it.
fn gen_str_reference_binding(
    raw_ident: &proc_macro2::Ident,
    capture: &TokenStream2,
    pat: &syn::Ident,
    ty: &syn::Type,
    missing_cap_err: &TokenStream2,
    quote_strip: &TokenStream2,
) -> TokenStream2 {
    quote! {
        let #raw_ident: &str = #capture.ok_or_else(|| #missing_cap_err)?;
        #quote_strip
        let #pat: #ty = #raw_ident;
    }
}

/// Generate code to bind an owned type argument from a regex capture using `.parse()`.
///
/// Optionally strips surrounding quotes if the hint requires it.
fn gen_parsed_type_binding(
    raw_ident: &proc_macro2::Ident,
    capture: &TokenStream2,
    pat: &syn::Ident,
    ty: &syn::Type,
    missing_cap_err: &TokenStream2,
    parse_err: &TokenStream2,
    quote_strip: &TokenStream2,
) -> TokenStream2 {
    quote! {
        let #raw_ident = #capture.ok_or_else(|| #missing_cap_err)?;
        #quote_strip
        let #pat: #ty = (#raw_ident).parse().map_err(|_| #parse_err)?;
    }
}

/// Context for parsing a single step argument from a regex capture.
#[derive(Copy, Clone)]
pub(super) struct ArgParseContext<'a> {
    /// The argument being parsed.
    pub(super) arg: &'a Arg,
    /// Index of this argument in the capture list.
    pub(super) idx: usize,
    /// Token stream representing the capture expression.
    pub(super) capture: &'a TokenStream2,
    /// Optional type hint (e.g., "string") for this placeholder.
    pub(super) hint: Option<&'a str>,
}

/// Generate parsing code for a single step argument from a regex capture.
///
/// Handles both borrowed `&str` references (direct assignment) and owned types
/// (using `.parse()`). When the placeholder has a `:string` type hint, the
/// surrounding quotes are stripped from the captured value before assignment
/// or parsing.
///
/// Returns the generated [`TokenStream2`] for declaring and initializing the
/// argument variable.
pub(super) fn gen_single_step_parse(ctx: ArgParseContext<'_>, meta: StepMeta<'_>) -> TokenStream2 {
    let ArgParseContext {
        arg,
        idx,
        capture,
        hint,
    } = ctx;
    let StepMeta { pattern, ident } = meta;
    let Arg::Step { pat, ty } = arg else {
        unreachable!("step argument vector must contain step args");
    };
    let raw_ident = format_ident!("__raw{}", idx);
    let missing_cap_err = step_error_tokens(
        &format_ident!("ExecutionError"),
        pattern,
        ident,
        &quote! {
            format!(
                "pattern '{}' missing capture for argument '{}'",
                #pattern,
                stringify!(#pat),
            )
        },
    );

    // Check if this placeholder has a :string hint requiring quote stripping
    let needs_quote_strip = rstest_bdd_patterns::requires_quote_stripping(hint);

    // Generate quote-stripping code only when needed; the error token stream is
    // constructed lazily to avoid building it when the hint does not require
    // quote stripping.
    let quote_strip = if needs_quote_strip {
        let malformed_quote_err = step_error_tokens(
            &format_ident!("ExecutionError"),
            pattern,
            ident,
            &quote! {
                format!(
                    "malformed quoted string for argument '{}': expected at least 2 characters, got '{}'",
                    stringify!(#pat),
                    #raw_ident,
                )
            },
        );
        gen_quote_strip(&raw_ident, &malformed_quote_err)
    } else {
        quote! {}
    };

    if is_str_reference(ty) {
        gen_str_reference_binding(&raw_ident, capture, pat, ty, &missing_cap_err, &quote_strip)
    } else {
        let parse_err = gen_parse_err(pattern, ident, pat, ty, &raw_ident);
        gen_parsed_type_binding(
            &raw_ident,
            capture,
            pat,
            ty,
            &missing_cap_err,
            &parse_err,
            &quote_strip,
        )
    }
}
