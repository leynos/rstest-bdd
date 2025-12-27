//! Single step argument parsing code generation.
//!
//! This module contains the logic for generating code that parses individual
//! step arguments from regex captures, including support for the `:string` type
//! hint which strips surrounding quotes.

use super::super::args::Arg;
use super::{StepMeta, is_str_reference, step_error_tokens};
use proc_macro2::TokenStream as TokenStream2;
use quote::{format_ident, quote};

/// Generate a token stream that strips surrounding quotes from a string slice.
///
/// The generated code reassigns `raw_ident` to a substring excluding the first
/// and last characters (the surrounding quotes).
fn gen_quote_strip(raw_ident: &proc_macro2::Ident) -> TokenStream2 {
    quote! {
        let #raw_ident: &str = &#raw_ident[1..#raw_ident.len() - 1];
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

    if is_str_reference(ty) {
        // Direct assignment for &str - no parsing needed
        let quote_strip = if needs_quote_strip {
            gen_quote_strip(&raw_ident)
        } else {
            quote! {}
        };
        quote! {
            let #raw_ident: &str = #capture.ok_or_else(|| #missing_cap_err)?;
            #quote_strip
            let #pat: #ty = #raw_ident;
        }
    } else {
        // Standard parse path for owned/parseable types
        let parse_err = step_error_tokens(
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
        );
        let quote_strip = if needs_quote_strip {
            gen_quote_strip(&raw_ident)
        } else {
            quote! {}
        };
        quote! {
            let #raw_ident = #capture.ok_or_else(|| #missing_cap_err)?;
            #quote_strip
            let #pat: #ty = (#raw_ident).parse().map_err(|_| #parse_err)?;
        }
    }
}
