//! Lint expectations attached to generated wrapper functions.

use proc_macro2::TokenStream as TokenStream2;
use quote::quote;

use super::WrapperKind;
use crate::return_classifier::ReturnKind;

/// Explains why generated wrapper functions need their expected lint list.
pub(super) const WRAPPER_EXPECT_REASON: &str = "rstest-bdd step wrapper pattern requires these \
                                                patterns for \\
     parameter extraction, Result normalization, and \\
     closure-based error handling";
/// Names the shadowing lint expected when string quotes are stripped.
pub(super) const LINT_SHADOW_REUSE: &str = "clippy::shadow_reuse";
/// Names the wrapper return lint expected for non-result step functions.
pub(super) const LINT_UNNECESSARY_WRAPS: &str = "clippy::unnecessary_wraps";
/// Names the string conversion lint expected for generated step structs.
pub(super) const LINT_STR_TO_STRING: &str = "clippy::str_to_string";
/// Names the method-call closure lint expected for captured placeholders.
pub(super) const LINT_REDUNDANT_CLOSURE_FOR_METHOD_CALLS: &str =
    "clippy::redundant_closure_for_method_calls";
/// Names the pass-by-value lint expected in generated wrappers.
pub(super) const LINT_NEEDLESS_PASS_BY_VALUE: &str = "clippy::needless_pass_by_value";
/// Names the closure lint expected in generated wrapper error handling.
pub(super) const LINT_REDUNDANT_CLOSURE: &str = "clippy::redundant_closure";
/// Names the lifetime lint expected in generated asynchronous wrappers.
pub(super) const LINT_NEEDLESS_LIFETIMES: &str = "clippy::needless_lifetimes";

/// Inputs used to select the lints expected from a generated wrapper.
#[derive(Copy, Clone)]
pub(super) struct WrapperLintConfig {
    /// Number of placeholder captures in the step pattern.
    pub(super) capture_count: usize,
    /// Whether the wrapper generates a step-struct declaration.
    pub(super) has_step_struct: bool,
    /// Whether ordinary step arguments strip placeholder quotes.
    pub(super) has_step_arg_quote_strip: bool,
    /// The return form of the wrapped step function.
    pub(super) return_kind: ReturnKind,
    /// Whether the wrapper is synchronous or asynchronous.
    pub(super) wrapper_kind: WrapperKind,
}

/// Select the Clippy lint names expected from the wrapper configuration.
fn wrapper_expect_lint_names(config: WrapperLintConfig) -> Vec<&'static str> {
    let mut lints = Vec::new();
    if config.has_step_arg_quote_strip {
        lints.push(LINT_SHADOW_REUSE);
    }
    if matches!(config.return_kind, ReturnKind::Unit | ReturnKind::Value) {
        lints.push(LINT_UNNECESSARY_WRAPS);
    }
    let has_placeholders = config.capture_count > 0;
    if config.has_step_struct && has_placeholders {
        lints.push(LINT_STR_TO_STRING);
    }
    if has_placeholders {
        lints.push(LINT_REDUNDANT_CLOSURE_FOR_METHOD_CALLS);
    }
    if config.wrapper_kind == WrapperKind::Async {
        lints.push(LINT_NEEDLESS_LIFETIMES);
    }
    lints.push(LINT_NEEDLESS_PASS_BY_VALUE);
    lints.push(LINT_REDUNDANT_CLOSURE);
    lints
}

/// Convert a Clippy lint name into the path used by an attribute.
fn lint_path_from_str(lint: &str) -> syn::Path {
    let mut segments = syn::punctuated::Punctuated::new();
    for segment in lint.split("::") {
        let ident = syn::Ident::new(segment, proc_macro2::Span::call_site());
        segments.push(syn::PathSegment::from(ident));
    }
    syn::Path {
        leading_colon: None,
        segments,
    }
}

/// Convert the selected expected lint names into attribute paths.
pub(super) fn wrapper_expect_lint_paths(config: WrapperLintConfig) -> Vec<syn::Path> {
    wrapper_expect_lint_names(config)
        .iter()
        .map(|lint| lint_path_from_str(lint))
        .collect()
}

/// Generate the expect attribute that records generated-wrapper lint intent.
pub(super) fn generate_expect_attribute(lint_paths: &[syn::Path]) -> TokenStream2 {
    if lint_paths.is_empty() {
        return TokenStream2::new();
    }
    quote! {
        #[expect(
            #(#lint_paths,)*
            reason = #WRAPPER_EXPECT_REASON
        )]
    }
}
