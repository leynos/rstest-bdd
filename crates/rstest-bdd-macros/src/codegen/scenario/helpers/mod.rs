//! Helpers for generating scenario code from parsed examples and steps.
//!
//! The module groups three concerns: [`steps`] converts parsed steps (with or
//! without placeholder substitution) into token vectors, [`case_attrs`] turns
//! Examples rows into `#[case(...)]` attributes, and the items here are shared
//! by both — the cell literal builder, the Examples-row predicate, and the
//! underscore-parameter expectation emitted for generated tests.

use proc_macro2::TokenStream as TokenStream2;
use quote::quote;

mod case_attrs;
mod steps;

pub(crate) use case_attrs::{generate_case_attrs, generate_indexed_case_attrs};
pub(crate) use steps::{ProcessedStepTokens, process_steps, process_steps_substituted};

/// Create a `LitStr` from an examples table cell.
fn cell_to_lit(value: &str) -> syn::LitStr {
    syn::LitStr::new(value, proc_macro2::Span::call_site())
}

/// Returns true when an Examples row contains at least one non-empty cell.
pub(crate) fn row_has_values(row: &[String]) -> bool { row.iter().any(|cell| !cell.is_empty()) }

/// Returns true when any parameter uses an underscore-prefixed identifier.
///
/// The single underscore `_` pattern is ignored. Only identifier patterns are
/// considered; non-identifier patterns are ignored.
///
/// # Examples
///
/// ```rust,ignore
/// use syn::parse_quote;
/// use crate::codegen::scenario::helpers::has_underscore_prefixed_params;
///
/// let sig: syn::Signature = parse_quote! { fn test(_state: State) };
/// assert!(has_underscore_prefixed_params(&sig));
/// ```
pub(crate) fn has_underscore_prefixed_params(sig: &syn::Signature) -> bool {
    sig.inputs.iter().any(|arg| {
        let syn::FnArg::Typed(pat_type) = arg else {
            return false;
        };
        let syn::Pat::Ident(pat_ident) = pat_type.pat.as_ref() else {
            return false;
        };
        let name = pat_ident.ident.to_string();
        name.starts_with('_') && name.len() > 1
    })
}

/// Emit a lint suppression attribute when underscore-prefixed parameters exist.
///
/// # Examples
///
/// ```rust,ignore
/// use syn::parse_quote;
/// use crate::codegen::scenario::helpers::generate_underscore_expect;
///
/// let sig: syn::Signature = parse_quote! { fn test(_state: State) };
/// let attr = generate_underscore_expect(&sig);
/// assert!(!attr.is_empty());
/// ```
pub(crate) fn generate_underscore_expect(sig: &syn::Signature) -> TokenStream2 {
    if has_underscore_prefixed_params(sig) {
        quote! {
            #[expect(
                clippy::used_underscore_binding,
                reason = "rstest-bdd scenario parameters are used by generated code"
            )]
        }
    } else {
        quote! {}
    }
}

#[cfg(test)]
mod tests {
    //! Unit tests for scenario code generation helpers.

    use rstest::rstest;
    use syn::parse_quote;

    use super::*;

    #[rstest]
    #[case(parse_quote! { fn test(_state: State) }, true)]
    #[case(parse_quote! { fn test(_: State) }, false)]
    #[case(parse_quote! { fn test(state: State) }, false)]
    #[case(parse_quote! { fn test(normal: i32, _unused: State) }, true)]
    #[case(parse_quote! { fn test() }, false)]
    fn detects_underscore_prefixed_params(#[case] sig: syn::Signature, #[case] expected: bool) {
        assert_eq!(
            has_underscore_prefixed_params(&sig),
            expected,
            "underscore-prefixed parameter detection disagreed with the expected verdict"
        );
    }
}
