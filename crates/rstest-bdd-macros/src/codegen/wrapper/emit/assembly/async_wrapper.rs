//! Async wrapper assembly for step wrapper emission.
//!
//! This module hosts the async wrapper generation path used for `async fn`
//! step definitions. The generated wrapper returns a `StepFuture` and uses a
//! future-aware unwind catcher to translate `skip!` and panics into the same
//! `StepExecution`/`StepError` flow as the synchronous wrappers.

use proc_macro2::TokenStream as TokenStream2;
use quote::quote;

/// Identifiers used in capture validation code generation.
#[derive(Clone, Copy)]
pub(super) struct CaptureValidationIdentifiers<'a> {
    pub(super) pattern: &'a proc_macro2::Ident,
    pub(super) text: &'a proc_macro2::Ident,
}

/// Error token streams for capture validation failures.
#[derive(Clone, Copy)]
pub(super) struct CaptureValidationErrors<'a> {
    pub(super) placeholder: &'a TokenStream2,
    pub(super) capture_mismatch: &'a TokenStream2,
}

/// Generate placeholder capture extraction and count validation tokens.
pub(super) fn generate_capture_validation(
    path: &TokenStream2,
    identifiers: CaptureValidationIdentifiers<'_>,
    expected: usize,
    errors: CaptureValidationErrors<'_>,
) -> TokenStream2 {
    let CaptureValidationIdentifiers {
        pattern: pattern_ident,
        text: text_ident,
    } = identifiers;
    let CaptureValidationErrors {
        placeholder: placeholder_err,
        capture_mismatch: capture_mismatch_err,
    } = errors;

    quote! {
        let captures = #path::extract_placeholders(&#pattern_ident, #text_ident.into())
            .map_err(|e| #placeholder_err)?;
        let expected: usize = #expected;
        if captures.len() != expected {
            return Err(#capture_mismatch_err);
        }
    }
}

/// Generate the unwind-catching match expression for async step wrappers.
pub(super) fn generate_unwind_handling(
    path: &TokenStream2,
    call_expr: &TokenStream2,
    exec_err: &TokenStream2,
    panic_err: &TokenStream2,
) -> TokenStream2 {
    quote! {
        match #path::__rstest_bdd_catch_unwind_future(async move { #call_expr }).await {
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
