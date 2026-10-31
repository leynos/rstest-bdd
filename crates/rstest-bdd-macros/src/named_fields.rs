//! Shared metadata and scalar conversion emission for named textual fields.
//!
//! Step captures and named data-table columns differ in lookup and missing-value
//! policy, but both normalize one string and convert it to a known Rust type.
//! This module owns that common conversion contract without imposing a shared
//! container or allocation strategy on either source.

use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::{ExprPath, Ident, LitStr, Type};

use crate::datatable::validation::is_string_type;

/// Scalar policies shared by named capture and named table-field bindings.
#[derive(Clone)]
pub(crate) struct ScalarConversion {
    /// Whether surrounding whitespace is removed before conversion.
    pub(crate) trim: bool,
    /// Optional custom parser replacing `FromStr` conversion.
    pub(crate) parse_with: Option<ExprPath>,
    /// Whether a boolean uses the data-table truthy vocabulary.
    pub(crate) truthy: bool,
}

impl ScalarConversion {
    /// Create a conversion policy with no normalization or custom parsing.
    pub(crate) fn plain() -> Self {
        Self {
            trim: false,
            parse_with: None,
            truthy: false,
        }
    }
}

/// Neutral specification for one Rust field bound from a named textual source.
pub(crate) struct NamedFieldSpec {
    /// Rust field receiving the converted value.
    pub(crate) rust_field: Ident,
    /// Name used by the textual source.
    pub(crate) source_name: LitStr,
    /// Type requested by the Rust field.
    pub(crate) target_type: Type,
    /// Shared normalization and scalar-conversion policy.
    pub(crate) conversion: ScalarConversion,
    /// Missing-value policy retained by the source adapter.
    pub(crate) missing: MissingValuePolicy,
}

/// Source-specific handling for missing named textual input.
pub(crate) enum MissingValuePolicy {
    /// Step captures are closed and every field must be present.
    Required,
    /// Data-table rows may provide optional or defaulted cells.
    DataTable {
        /// Whether a missing cell yields `None`.
        optional: bool,
        /// Whether a missing cell has an explicit fallback.
        has_default: bool,
    },
}

/// Emit conversion of `raw` into `target_type` using the shared policy.
pub(crate) fn scalar_parser_expression(
    conversion: &ScalarConversion,
    raw: TokenStream2,
    target_type: &Type,
    runtime: &TokenStream2,
) -> TokenStream2 {
    let normalized = if conversion.trim {
        quote! {{ let normalized = (#raw).trim(); normalized }}
    } else {
        raw
    };
    match &conversion.parse_with {
        Some(parser) => quote! { #parser(#normalized) },
        None if conversion.truthy => quote! { #runtime::datatable::truthy_bool(#normalized) },
        None if is_string_type(target_type) => {
            quote! { Ok::<#target_type, ::core::convert::Infallible>((#normalized).to_owned()) }
        }
        None => quote! { (#normalized).parse::<#target_type>() },
    }
}

/// Determine whether generated code needs a `FromStr` bound.
pub(crate) fn requires_fromstr(conversion: &ScalarConversion, target_type: &Type) -> bool {
    conversion.parse_with.is_none() && !conversion.truthy && !is_string_type(target_type)
}
