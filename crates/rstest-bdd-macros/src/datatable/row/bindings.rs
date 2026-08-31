//! Builds binding expressions for `#[derive(DataTableRow)]` fields.
//!
//! This module normalizes how generated code fetches cell values from the
//! runtime `DataTable`, including graceful handling for optional and defaulted
//! fields. Optional members yield `None` when the source column is absent,
//! while defaults fall back to either `Default::default()` or a caller-supplied
//! function. All other fields bubble errors straight back to the caller so
//! derivations retain the existing failure semantics.

use proc_macro2::TokenStream as TokenStream2;
use quote::{format_ident, quote};
use syn::Type;

use crate::{
    datatable::{
        config::{DefaultValue, FieldSpec},
        parser::accessor_expr,
    },
    named_fields::MissingValuePolicy,
};

/// Provides the internal `build_field_binding` operation.
pub(crate) fn build_field_binding(
    index: usize,
    field: &FieldSpec,
    runtime: &TokenStream2,
) -> TokenStream2 {
    let binding_ident = binding_ident(index, field);
    let accessor = accessor_expr(field, runtime, index);
    let missing_pattern = missing_error_pattern(runtime);
    let (is_optional, _has_default) = missing_value_policy(field);
    let on_missing = missing_value_fallback(field, is_optional);

    on_missing.map_or_else(
        || build_required_binding(&binding_ident, &accessor),
        |on_missing| {
            build_recovering_binding(
                &binding_ident,
                &accessor,
                &missing_pattern,
                &on_missing,
                is_optional,
            )
        },
    )
}

/// Select the local identifier that receives one parsed field value.
fn binding_ident(index: usize, field: &FieldSpec) -> syn::Ident {
    field
        .named
        .as_ref()
        .map(|named| named.rust_field.clone())
        .or_else(|| field.ident.clone())
        .unwrap_or_else(|| format_ident!("__field_{index}"))
}

/// Build the fallback used only when a table column or cell is absent.
fn missing_value_fallback(field: &FieldSpec, is_optional: bool) -> Option<TokenStream2> {
    if is_optional {
        return Some(quote! { None });
    }

    field
        .config
        .default
        .as_ref()
        .map(|default| build_default_expr(default, &field.ty))
}

/// Emit a required-field binding that propagates lookup and conversion errors.
fn build_required_binding(binding_ident: &syn::Ident, accessor: &TokenStream2) -> TokenStream2 {
    quote! {
        let #binding_ident = #accessor?;
    }
}

/// Emit an optional or defaulted binding that recovers only missing table data.
fn build_recovering_binding(
    binding_ident: &syn::Ident,
    accessor: &TokenStream2,
    missing_pattern: &TokenStream2,
    on_missing: &TokenStream2,
    is_optional: bool,
) -> TokenStream2 {
    let on_success = if is_optional {
        quote! { Some(value) }
    } else {
        quote! { value }
    };
    quote! {
        let #binding_ident = match #accessor {
            Ok(value) => #on_success,
            Err(err) => match err {
                #missing_pattern => #on_missing,
                _ => return Err(err),
            },
        };
    }
}

/// Read source-specific missing-value policy without changing tuple semantics.
fn missing_value_policy(field: &FieldSpec) -> (bool, bool) {
    match field.named.as_ref().map(|named| &named.missing) {
        Some(MissingValuePolicy::DataTable {
            optional,
            has_default,
        }) => (*optional, *has_default),
        Some(MissingValuePolicy::Required) | None => {
            (field.config.optional, field.config.default.is_some())
        }
    }
}

/// Provides the internal `build_default_expr` operation.
fn build_default_expr(default: &DefaultValue, ty: &Type) -> TokenStream2 {
    match default {
        DefaultValue::Trait => quote! { <#ty as ::core::default::Default>::default() },
        DefaultValue::Function(path) => quote! { #path() },
    }
}

/// Provides the internal `missing_error_pattern` operation.
fn missing_error_pattern(runtime: &TokenStream2) -> TokenStream2 {
    quote! {
        #runtime::datatable::DataTableError::MissingColumn { .. }
        | #runtime::datatable::DataTableError::MissingCell { .. }
    }
}

#[cfg(test)]
mod tests {
    //! Token-level regression coverage for data-table field bindings.

    use quote::quote;
    use syn::parse_quote;

    use super::*;
    use crate::{
        datatable::config::{Accessor, FieldConfig},
        named_fields::{MissingValuePolicy, NamedFieldSpec, ScalarConversion},
    };

    fn field_spec(ident: Option<syn::Ident>) -> FieldSpec {
        FieldSpec {
            ident,
            ty: parse_quote!(String),
            inner_ty: parse_quote!(String),
            config: FieldConfig::new(Accessor::Column {
                name: "field".to_owned(),
            }),
            named: None,
        }
    }

    fn binding_tokens(field: &FieldSpec) -> String {
        build_field_binding(0, field, &quote!(::rstest_bdd)).to_string()
    }

    #[test]
    fn required_field_propagates_accessor_errors() {
        let tokens = binding_tokens(&field_spec(Some(parse_quote!(field))));

        assert!(tokens.contains("let field ="));
        assert!(tokens.contains('?'));
    }

    #[test]
    fn optional_field_recovers_missing_cells() {
        let mut field = field_spec(Some(parse_quote!(field)));
        field.config.optional = true;
        let tokens = binding_tokens(&field);

        assert!(tokens.contains("Some (value)"));
        assert!(tokens.contains("MissingColumn"));
        assert!(tokens.contains("MissingCell"));
        assert!(tokens.contains("None"));
    }

    #[test]
    fn trait_default_recovers_missing_cells() {
        let mut field = field_spec(Some(parse_quote!(field)));
        field.config.default = Some(DefaultValue::Trait);
        let tokens = binding_tokens(&field);

        assert!(tokens.contains("Default > :: default"));
        assert!(tokens.contains("Ok (value) => value"));
    }

    #[test]
    fn function_default_recovers_missing_cells() {
        let mut field = field_spec(Some(parse_quote!(field)));
        field.config.default = Some(DefaultValue::Function(parse_quote!(provider)));
        let tokens = binding_tokens(&field);

        assert!(tokens.contains("provider ()"));
        assert!(tokens.contains("MissingColumn"));
        assert!(tokens.contains("MissingCell"));
    }

    #[test]
    fn named_field_prefers_the_shared_rust_identifier() {
        let mut field = field_spec(None);
        field.named = Some(NamedFieldSpec {
            rust_field: parse_quote!(renamed),
            source_name: syn::LitStr::new("source", proc_macro2::Span::call_site()),
            target_type: parse_quote!(String),
            conversion: ScalarConversion::plain(),
            missing: MissingValuePolicy::Required,
        });
        let tokens = binding_tokens(&field);

        assert!(tokens.contains("let renamed ="));
    }

    #[test]
    fn tuple_field_uses_generated_binding_identifier() {
        let tokens = binding_tokens(&field_spec(None));

        assert!(tokens.contains("let __field_0 ="));
    }
}
