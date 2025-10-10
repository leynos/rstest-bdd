//! Code generation for the `DataTableRow` derive macro.
//!
//! The expander validates the annotated struct and emits a runtime
//! implementation capable of parsing rows into strongly typed values.

use proc_macro::TokenStream;
use proc_macro2::{Ident, TokenStream as TokenStream2};
use quote::{format_ident, quote};
use syn::{
    Attribute, Data, DataStruct, DeriveInput, ExprPath, Field, Fields, GenericArgument, Generics,
    LitStr, PathArguments, Token, Type, TypePath, parse_macro_input, spanned::Spanned,
};

use crate::codegen::rstest_bdd_path;

use super::rename::RenameRule;

pub(crate) fn expand(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    match expand_inner(&input) {
        Ok(tokens) => tokens.into(),
        Err(err) => err.into_compile_error().into(),
    }
}

struct StructConfig {
    rename_rule: Option<RenameRule>,
}

struct FieldConfig {
    accessor: Accessor,
    optional: bool,
    default: Option<DefaultValue>,
    parse_with: Option<ExprPath>,
    truthy: bool,
    trim: bool,
}

enum DefaultValue {
    Trait,
    Function(ExprPath),
}

#[derive(Clone)]
enum Accessor {
    Column { name: String },
    Index { position: usize },
}

struct FieldSpec {
    ident: Option<Ident>,
    ty: Type,
    inner_ty: Type,
    config: FieldConfig,
}

fn expand_inner(input: &DeriveInput) -> syn::Result<TokenStream2> {
    let Data::Struct(DataStruct { fields, .. }) = &input.data else {
        return Err(syn::Error::new(
            input.span(),
            "#[derive(DataTableRow)] only supports structs",
        ));
    };
    let struct_config = parse_struct_config(&input.attrs)?;
    let field_specs = collect_fields(fields, &struct_config)?;
    let runtime = rstest_bdd_path();
    let requires_header = field_specs
        .iter()
        .any(|field| matches!(field.config.accessor, Accessor::Column { .. }));
    let bindings: Vec<_> = field_specs
        .iter()
        .enumerate()
        .map(|(index, field)| build_field_binding(index, field, &runtime))
        .collect();
    let construct = build_constructor(&field_specs);
    let ident = &input.ident;
    let generics = augment_generics(&input.generics, &field_specs);
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

    Ok(quote! {
        impl #impl_generics #runtime::datatable::DataTableRow for #ident #ty_generics #where_clause {
            const REQUIRES_HEADER: bool = #requires_header;

            fn parse_row(row: #runtime::datatable::RowSpec<'_>) -> Result<Self, #runtime::datatable::DataTableError> {
                #(#bindings)*
                Ok(#construct)
            }
        }
    })
}

fn parse_struct_config(attrs: &[Attribute]) -> syn::Result<StructConfig> {
    let mut rename_rule = None;
    for attr in attrs {
        if !attr.path().is_ident("datatable") {
            continue;
        }
        attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("rename_all") {
                let value: LitStr = meta.value()?.parse()?;
                let rule = RenameRule::try_from(&value)?;
                if rename_rule.replace(rule).is_some() {
                    return Err(meta.error("duplicate rename_all attribute"));
                }
                Ok(())
            } else {
                Err(meta.error("unsupported datatable attribute"))
            }
        })?;
    }
    Ok(StructConfig { rename_rule })
}

fn collect_fields(fields: &Fields, config: &StructConfig) -> syn::Result<Vec<FieldSpec>> {
    match fields {
        Fields::Named(named) => named
            .named
            .iter()
            .map(|field| build_named_field(field, config))
            .collect(),
        Fields::Unnamed(unnamed) => unnamed
            .unnamed
            .iter()
            .enumerate()
            .map(|(index, field)| build_unnamed_field(field, index))
            .collect(),
        Fields::Unit => Err(syn::Error::new(
            fields.span(),
            "#[derive(DataTableRow)] does not support unit structs",
        )),
    }
}

fn build_named_field(field: &Field, config: &StructConfig) -> syn::Result<FieldSpec> {
    let ident = field
        .ident
        .clone()
        .ok_or_else(|| syn::Error::new(field.span(), "named field missing ident"))?;
    let default_column = config
        .rename_rule
        .map_or_else(|| ident.to_string(), |rule| rule.apply(&ident.to_string()));
    let accessor = Accessor::Column {
        name: default_column,
    };
    build_field_spec(Some(ident), field, accessor)
}

fn build_unnamed_field(field: &Field, index: usize) -> syn::Result<FieldSpec> {
    let accessor = Accessor::Index { position: index };
    build_field_spec(None, field, accessor)
}

fn build_field_spec(
    ident: Option<Ident>,
    field: &Field,
    base_accessor: Accessor,
) -> syn::Result<FieldSpec> {
    let config = parse_field_attributes(&field.attrs, base_accessor)?;
    let (is_option, inner_ty) = option_inner_type(&field.ty)?;
    validate_field_config(&config, is_option, &inner_ty, field.span())?;
    Ok(FieldSpec {
        ident,
        ty: field.ty.clone(),
        inner_ty,
        config,
    })
}

fn parse_field_attributes(
    attrs: &[Attribute],
    base_accessor: Accessor,
) -> syn::Result<FieldConfig> {
    let mut config = FieldConfig::new(base_accessor);
    for attr in attrs {
        if !attr.path().is_ident("datatable") {
            continue;
        }
        attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("column") {
                let value: LitStr = meta.value()?.parse()?;
                config.accessor = Accessor::Column {
                    name: value.value(),
                };
                Ok(())
            } else if meta.path.is_ident("optional") {
                config.optional = true;
                Ok(())
            } else if meta.path.is_ident("default") {
                if meta.input.peek(Token![=]) {
                    let path: ExprPath = meta.value()?.parse()?;
                    config.default = Some(DefaultValue::Function(path));
                } else {
                    config.default = Some(DefaultValue::Trait);
                }
                Ok(())
            } else if meta.path.is_ident("parse_with") {
                let path: ExprPath = meta.value()?.parse()?;
                if config.parse_with.replace(path).is_some() {
                    return Err(meta.error("duplicate parse_with attribute"));
                }
                Ok(())
            } else if meta.path.is_ident("truthy") {
                config.truthy = true;
                Ok(())
            } else if meta.path.is_ident("trim") {
                config.trim = true;
                Ok(())
            } else {
                Err(meta.error("unsupported datatable attribute"))
            }
        })?;
    }
    Ok(config)
}

fn validate_field_config(
    config: &FieldConfig,
    is_option: bool,
    inner_ty: &Type,
    span: proc_macro2::Span,
) -> syn::Result<()> {
    if config.optional && config.default.is_some() {
        return Err(syn::Error::new(
            span,
            "optional fields cannot specify a default",
        ));
    }
    if config.truthy && config.parse_with.is_some() {
        return Err(syn::Error::new(
            span,
            "truthy and parse_with are mutually exclusive",
        ));
    }
    if config.optional && !is_option {
        return Err(syn::Error::new(
            span,
            "#[datatable(optional)] requires an Option<T> field",
        ));
    }
    if is_option && config.default.is_some() {
        return Err(syn::Error::new(
            span,
            "Option<T> fields cannot define a default value",
        ));
    }
    if config.truthy && !is_bool_type(inner_ty) {
        return Err(syn::Error::new(
            span,
            "#[datatable(truthy)] requires a bool field",
        ));
    }
    Ok(())
}

impl FieldConfig {
    fn new(accessor: Accessor) -> Self {
        Self {
            accessor,
            optional: false,
            default: None,
            parse_with: None,
            truthy: false,
            trim: false,
        }
    }
}

fn option_inner_type(ty: &Type) -> syn::Result<(bool, Type)> {
    if let Type::Path(TypePath { path, .. }) = ty {
        if let Some(segment) = path.segments.last() {
            if segment.ident == "Option" {
                if let PathArguments::AngleBracketed(args) = &segment.arguments {
                    if let Some(GenericArgument::Type(inner)) = args.args.first() {
                        return Ok((true, inner.clone()));
                    }
                }
                return Err(syn::Error::new(
                    ty.span(),
                    "Option<T> must specify an inner type",
                ));
            }
        }
    }
    Ok((false, ty.clone()))
}

fn build_field_binding(index: usize, field: &FieldSpec, runtime: &TokenStream2) -> TokenStream2 {
    let binding_ident = field
        .ident
        .clone()
        .unwrap_or_else(|| format_ident!("__field_{index}"));
    let accessor = accessor_expr(field, runtime, index);
    if field.config.optional {
        build_optional_field_binding(binding_ident, accessor, runtime)
    } else if let Some(default) = &field.config.default {
        let default_expr = build_default_expr(default, &field.ty);
        build_field_binding_with_default(binding_ident, accessor, default_expr, runtime)
    } else {
        build_required_field_binding(binding_ident, accessor)
    }
}

// These helpers take owned tokens to satisfy the refactoring contract while
// keeping the call sites ergonomic, so suppress Clippy's pass-by-value lint.
#[expect(
    clippy::needless_pass_by_value,
    reason = "refactoring brief mandates these helpers take ownership"
)]
fn build_optional_field_binding(
    binding_ident: Ident,
    accessor: TokenStream2,
    runtime: &TokenStream2,
) -> TokenStream2 {
    let missing_pattern = missing_error_pattern(runtime);
    quote! {
        let #binding_ident = match #accessor {
            Ok(value) => Some(value),
            Err(err) => match err {
                #missing_pattern => None,
                _ => return Err(err),
            },
        };
    }
}

#[expect(
    clippy::needless_pass_by_value,
    reason = "refactoring brief mandates these helpers take ownership"
)]
fn build_field_binding_with_default(
    binding_ident: Ident,
    accessor: TokenStream2,
    default_expr: TokenStream2,
    runtime: &TokenStream2,
) -> TokenStream2 {
    let missing_pattern = missing_error_pattern(runtime);
    quote! {
        let #binding_ident = match #accessor {
            Ok(value) => value,
            Err(err) => match err {
                #missing_pattern => #default_expr,
                _ => return Err(err),
            },
        };
    }
}

#[expect(
    clippy::needless_pass_by_value,
    reason = "refactoring brief mandates these helpers take ownership"
)]
fn build_required_field_binding(binding_ident: Ident, accessor: TokenStream2) -> TokenStream2 {
    quote! {
        let #binding_ident = #accessor?;
    }
}

fn build_default_expr(default: &DefaultValue, ty: &Type) -> TokenStream2 {
    match default {
        DefaultValue::Trait => quote! { <#ty as ::core::default::Default>::default() },
        DefaultValue::Function(path) => quote! { #path() },
    }
}

fn missing_error_pattern(runtime: &TokenStream2) -> TokenStream2 {
    quote! {
        #runtime::datatable::DataTableError::MissingColumn { .. }
        | #runtime::datatable::DataTableError::MissingCell { .. }
    }
}

fn accessor_expr(field: &FieldSpec, runtime: &TokenStream2, index: usize) -> TokenStream2 {
    let closure = parser_closure(&field.config, &field.inner_ty, runtime, index);
    match &field.config.accessor {
        Accessor::Column { name, .. } => {
            quote! { row.parse_column_with(#name, #closure) }
        }
        Accessor::Index { position, .. } => {
            let pos = syn::Index::from(*position);
            quote! { row.parse_with(#pos, #closure) }
        }
    }
}

fn parser_closure(
    config: &FieldConfig,
    target_ty: &Type,
    runtime: &TokenStream2,
    index: usize,
) -> TokenStream2 {
    let value_ident = format_ident!("cell_{index}");
    let mut statements = Vec::new();
    let mut current = quote! { #value_ident };
    if config.trim {
        let trimmed = format_ident!("trimmed_{index}");
        statements.push(quote! { let #trimmed = #current.trim(); });
        current = quote! { #trimmed };
    }
    let parse_expr = config.parse_with.as_ref().map_or_else(
        || {
            if config.truthy {
                quote! { #runtime::datatable::truthy_bool(#current) }
            } else if is_string_type(target_ty) {
                quote! { Ok::<#target_ty, ::core::convert::Infallible>(#current.to_owned()) }
            } else {
                quote! { #current.parse::<#target_ty>() }
            }
        },
        |parser| quote! { #parser(#current) },
    );
    quote! {
        |#value_ident| {
            #(#statements)*
            #parse_expr
        }
    }
}

fn is_string_type(ty: &Type) -> bool {
    if let Type::Path(path) = ty {
        if let Some(segment) = path.path.segments.last() {
            return segment.ident == "String" && matches!(segment.arguments, PathArguments::None);
        }
    }
    false
}

fn is_bool_type(ty: &Type) -> bool {
    if let Type::Path(path) = ty {
        if let Some(segment) = path.path.segments.last() {
            return segment.ident == "bool" && matches!(segment.arguments, PathArguments::None);
        }
    }
    false
}

fn build_constructor(fields: &[FieldSpec]) -> TokenStream2 {
    fields
        .iter()
        .map(|field| field.ident.as_ref())
        .collect::<Option<Vec<_>>>()
        .map_or_else(
            || {
                let inits = fields.iter().enumerate().map(|(index, field)| {
                    let ident = field
                        .ident
                        .clone()
                        .unwrap_or_else(|| format_ident!("__field_{index}"));
                    quote! { #ident }
                });
                quote! { Self(#(#inits),*) }
            },
            |idents| {
                let inits = idents.iter().map(|ident| quote! { #ident });
                quote! { Self { #(#inits),* } }
            },
        )
}

fn augment_generics(generics: &Generics, fields: &[FieldSpec]) -> Generics {
    let mut generics = generics.clone();
    let where_clause = generics.make_where_clause();
    for field in fields {
        if needs_from_str_bound(&field.config, &field.inner_ty) {
            let ty = &field.inner_ty;
            where_clause.predicates.push(syn::parse_quote! {
                #ty: ::core::str::FromStr,
            });
            where_clause.predicates.push(syn::parse_quote! {
                <#ty as ::core::str::FromStr>::Err: ::std::error::Error + Send + Sync + 'static,
            });
        }
    }
    generics
}

fn needs_from_str_bound(config: &FieldConfig, inner_ty: &Type) -> bool {
    config.parse_with.is_none() && !config.truthy && !is_string_type(inner_ty)
}
