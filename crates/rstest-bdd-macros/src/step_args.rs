//! Expansion logic for `#[derive(StepArgs)]`.
//!
//! The derive macro targets structs with named fields and generates
//! implementations for [`rstest_bdd::step_args::StepArgs`] that bind captures
//! by their placeholder names. Fields use [`FromStr`] unless they configure a
//! custom parser, enabling the runtime wrapper to construct the struct without
//! declaration-order coupling.
use std::collections::HashSet;

use proc_macro::TokenStream;
use proc_macro2::{Span, TokenStream as TokenStream2};
use quote::quote;
use syn::{Attribute, DeriveInput, LitStr, parse_quote, spanned::Spanned};

use crate::named_fields::{
    MissingValuePolicy,
    NamedFieldSpec,
    ScalarConversion,
    requires_fromstr,
    scalar_parser_expression,
};
mod field_attributes;
/// Expand the `StepArgs` derive implementation.
pub(crate) fn derive(input: TokenStream) -> TokenStream {
    let input = syn::parse_macro_input!(input as DeriveInput);
    match expand(input) {
        Ok(tokens) => tokens.into(),
        Err(err) => err.into_compile_error().into(),
    }
}
/// Validate the input and generate its `StepArgs` implementations.
fn expand(input: DeriveInput) -> syn::Result<TokenStream2> {
    let DeriveInput {
        ident,
        generics,
        data,
        attrs,
        ..
    } = input;
    let syn::Data::Struct(struct_data) = data else {
        return Err(syn::Error::new(
            ident.span(),
            "StepArgs can only be derived for structs",
        ));
    };
    let syn::Fields::Named(fields) = struct_data.fields else {
        return Err(syn::Error::new(
            struct_data.struct_token.span(),
            "StepArgs requires named struct fields",
        ));
    };
    expand_named_struct(&ident, generics, fields, &attrs)
}
/// Shared metadata for one named step-argument field.
type FieldInfo = NamedFieldSpec;
/// Collect and validate metadata for all named fields.
fn collect_field_info(
    ident: &syn::Ident,
    fields: syn::FieldsNamed,
    attrs: &[Attribute],
) -> syn::Result<Vec<FieldInfo>> {
    let rename_rule = parse_rename_rule(attrs)?;
    let mut seen = HashSet::new();
    let mut field_infos = Vec::new();
    for field in fields.named {
        let field_span = field.span();
        let field_ident = field
            .ident
            .ok_or_else(|| syn::Error::new(field_span, "named field missing identifier"))?;
        let default = rename_rule.as_ref().map_or_else(
            || field_ident.to_string(),
            |rule| rule.apply(&field_ident.to_string()),
        );
        let config = parse_step_field(&field.attrs)?;
        let source = config.placeholder.unwrap_or(default);
        if !seen.insert(source.clone()) {
            return Err(syn::Error::new(
                field_ident.span(),
                format!("duplicate StepArgs placeholder `{source}`"),
            ));
        }
        field_infos.push(FieldInfo {
            rust_field: field_ident,
            source_name: syn::LitStr::new(&source, Span::call_site()),
            target_type: field.ty,
            conversion: ScalarConversion {
                trim: config.trim,
                parse_with: config.parse_with,
                truthy: false,
            },
            missing: MissingValuePolicy::Required,
        });
    }

    if field_infos.is_empty() {
        return Err(syn::Error::new(
            ident.span(),
            "StepArgs structs must define at least one field",
        ));
    }
    Ok(field_infos)
}
/// Parse the struct-level `step_args(rename_all = "...")` setting.
fn parse_rename_rule(
    attrs: &[Attribute],
) -> syn::Result<Option<crate::datatable::rename::RenameRule>> {
    let mut rename_rule = None;
    for attr in attrs
        .iter()
        .filter(|attr| attr.path().is_ident("step_args"))
    {
        attr.parse_nested_meta(|meta| {
            if !meta.path.is_ident("rename_all") {
                return Err(meta.error("unsupported step_args attribute"));
            }
            let value: LitStr = meta.value()?.parse()?;
            let rule = crate::datatable::rename::RenameRule::try_from(&value)?;
            if rename_rule.replace(rule).is_some() {
                return Err(meta.error("duplicate rename_all attribute"));
            }
            Ok(())
        })?;
    }
    Ok(rename_rule)
}
/// Parsed `#[step_args(...)]` options for one struct field.
pub(super) struct StepFieldConfig {
    /// Explicit placeholder name, when it differs from the field name.
    pub(super) placeholder: Option<String>,
    /// Whether surrounding whitespace is removed before parsing.
    pub(super) trim: bool,
    /// Parser used instead of [`FromStr`] conversion.
    pub(super) parse_with: Option<syn::ExprPath>,
}
/// Parse field-level `step_args` configuration.
fn parse_step_field(attrs: &[Attribute]) -> syn::Result<StepFieldConfig> {
    let mut config = StepFieldConfig {
        placeholder: None,
        trim: false,
        parse_with: None,
    };
    for attr in attrs
        .iter()
        .filter(|attr| attr.path().is_ident("step_args"))
    {
        attr.parse_nested_meta(|meta| {
            field_attributes::process_step_field_meta_item(&meta, &mut config)
        })?;
    }
    Ok(config)
}
/// Add `FromStr` bounds for each generated field parser.
fn add_fromstr_bounds(generics: &mut syn::Generics, field_infos: &[FieldInfo]) {
    let where_clause = generics.make_where_clause();
    for info in field_infos
        .iter()
        .filter(|info| requires_fromstr(&info.conversion, &info.target_type))
    {
        let ty = &info.target_type;
        where_clause
            .predicates
            .push(parse_quote!(#ty: ::core::str::FromStr));
    }
}
/// Generate named field parsing expressions and construction metadata.
fn generate_field_parsing<'a>(
    field_infos: &'a [FieldInfo],
    runtime: &TokenStream2,
) -> (
    Vec<TokenStream2>,
    Vec<&'a syn::Ident>,
    Vec<syn::LitStr>,
    usize,
) {
    let parse_blocks: Vec<_> = field_infos
        .iter()
        .map(|info| {
            let ident = &info.rust_field;
            let ty = &info.target_type;
            let name = &info.source_name;
            let parse = scalar_parser_expression(
                &info.conversion,
                quote! { __rstest_bdd_raw.value.as_str() },
                ty,
                runtime,
            );
            let parse_failure = if info.conversion.parse_with.is_some() {
                quote! {
                    #runtime::step_args::StepArgsError::custom_parse_failure(
                        stringify!(#ident),
                        &__rstest_bdd_raw.value,
                        _error,
                    )
                }
            } else {
                quote! {
                    #runtime::step_args::StepArgsError::parse_failure(
                        stringify!(#ident),
                        &__rstest_bdd_raw.value,
                    )
                }
            };
            quote! {
                let __rstest_bdd_raw = __rstest_bdd_captures
                    .iter()
                    .find(|capture| capture.name == #name)
                    .ok_or_else(|| #runtime::step_args::StepArgsError::missing_field(
                        stringify!(#ident),
                        #name,
                    ))?;
                let #ident: #ty = match #parse {
                    Ok(value) => value,
                    Err(_error) => {
                        return Err(#parse_failure);
                    }
                };
            }
        })
        .collect();
    let field_idents = field_infos.iter().map(|info| &info.rust_field).collect();
    let field_name_literals = field_infos
        .iter()
        .map(|info| info.source_name.clone())
        .collect();
    let field_count = field_infos.len();
    (parse_blocks, field_idents, field_name_literals, field_count)
}
/// Inputs used to generate the `StepArgs` and `TryFrom` implementations.
struct TraitImplParams<'a> {
    /// The source struct identifier.
    ident: &'a syn::Ident,
    /// Generics used on the generated implementation.
    impl_generics: syn::ImplGenerics<'a>,
    /// Type generics used on the generated implementation.
    ty_generics: syn::TypeGenerics<'a>,
    /// Optional where clause used on the generated implementation.
    where_clause: Option<&'a syn::WhereClause>,
    /// Number of captured fields expected by the implementation.
    field_count: usize,
    /// Generated field-name literals exposed by `StepArgs`.
    field_name_literals: &'a [syn::LitStr],
    /// Generated expressions that parse each capture.
    parse_fields: &'a [TokenStream2],
    /// Generated expression constructing the final value.
    construct: TokenStream2,
    /// Path to the runtime crate used by generated code.
    runtime: TokenStream2,
}

/// Generate the trait implementations for a named step-argument struct.
fn generate_trait_impl(ctx: TraitImplParams<'_>) -> TokenStream2 {
    let TraitImplParams {
        ident,
        impl_generics,
        ty_generics,
        where_clause,
        field_count,
        field_name_literals,
        parse_fields,
        construct,
        runtime,
    } = ctx;

    quote! {
        impl #impl_generics #runtime::step_args::StepArgs for #ident #ty_generics #where_clause {
            const FIELD_COUNT: usize = #field_count;
            const FIELD_NAMES: &'static [&'static str] = &[#(#field_name_literals),*];

            fn from_captures(values: Vec<String>) -> Result<Self, #runtime::step_args::StepArgsError> {
                if values.len() != Self::FIELD_NAMES.len() {
                    return Err(#runtime::step_args::StepArgsError::count_mismatch(
                        Self::FIELD_NAMES.len(),
                        values.len(),
                    ));
                }
                let __rstest_bdd_captures = Self::FIELD_NAMES
                    .iter()
                    .copied()
                    .zip(values)
                    .map(|(name, value)| #runtime::step_args::StepCapture { name, value })
                    .collect();
                Self::from_named_captures(__rstest_bdd_captures)
            }

            fn from_named_captures(
                __rstest_bdd_captures: Vec<#runtime::step_args::StepCapture>,
            ) -> Result<Self, #runtime::step_args::StepArgsError> {
                if __rstest_bdd_captures.len() != Self::FIELD_COUNT {
                    return Err(#runtime::step_args::StepArgsError::count_mismatch(
                        Self::FIELD_COUNT,
                        __rstest_bdd_captures.len(),
                    ));
                }
                if let Some(capture) = __rstest_bdd_captures
                    .iter()
                    .find(|capture| !Self::FIELD_NAMES.contains(&capture.name))
                {
                    return Err(#runtime::step_args::StepArgsError::unconsumed_capture(capture.name));
                }
                if let Some(capture) = __rstest_bdd_captures.iter().enumerate().find_map(|(index, capture)| {
                    __rstest_bdd_captures
                        .iter()
                        .take(index)
                        .any(|earlier| earlier.name == capture.name)
                        .then_some(capture)
                }) {
                    return Err(#runtime::step_args::StepArgsError::duplicate_capture(capture.name));
                }
                #(#parse_fields)*
                Ok(#construct)
            }
        }

        impl #impl_generics ::std::convert::TryFrom<Vec<String>> for #ident #ty_generics #where_clause {
            type Error = #runtime::step_args::StepArgsError;

            fn try_from(value: Vec<String>) -> Result<Self, Self::Error> {
                <Self as #runtime::step_args::StepArgs>::from_captures(value)
            }
        }
    }
}

/// Generate implementations for a named struct after validating its fields.
fn expand_named_struct(
    ident: &syn::Ident,
    mut generics: syn::Generics,
    fields: syn::FieldsNamed,
    attrs: &[Attribute],
) -> syn::Result<TokenStream2> {
    let runtime = crate::codegen::rstest_bdd_path();
    let field_infos = collect_field_info(ident, fields, attrs)?;

    add_fromstr_bounds(&mut generics, &field_infos);
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();
    let (parse_fields, field_idents, field_name_literals, field_count) =
        generate_field_parsing(&field_infos, &runtime);

    let construct = quote! { Self { #(#field_idents),* } };

    let ctx = TraitImplParams {
        ident,
        impl_generics,
        ty_generics,
        where_clause,
        field_count,
        field_name_literals: &field_name_literals,
        parse_fields: &parse_fields,
        construct,
        runtime,
    };

    Ok(generate_trait_impl(ctx))
}

#[cfg(test)]
mod tests;
