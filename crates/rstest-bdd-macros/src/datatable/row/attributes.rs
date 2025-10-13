use proc_macro2::{Ident, Span};
use syn::{Attribute, ExprPath, Field, Fields, LitStr, Token, Type, spanned::Spanned};

use crate::datatable::config::{Accessor, DefaultValue, FieldConfig, FieldSpec, StructConfig};
use crate::datatable::rename::RenameRule;
use crate::datatable::validation::{is_bool_type, option_inner_type};

pub(crate) fn parse_struct_config(attrs: &[Attribute]) -> syn::Result<StructConfig> {
    let mut rename_rule = None;
    for attr in attrs
        .iter()
        .filter(|attr| attr.path().is_ident("datatable"))
    {
        process_datatable_attr(attr, &mut rename_rule)?;
    }
    Ok(StructConfig { rename_rule })
}

fn process_datatable_attr(
    attr: &Attribute,
    rename_rule: &mut Option<RenameRule>,
) -> syn::Result<()> {
    attr.parse_nested_meta(|meta| parse_rename_all_from_meta(&meta, rename_rule))
}

fn parse_rename_all_from_meta(
    meta: &syn::meta::ParseNestedMeta,
    rename_rule: &mut Option<RenameRule>,
) -> syn::Result<()> {
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
}

pub(crate) fn collect_fields(
    fields: &Fields,
    config: &StructConfig,
) -> syn::Result<Vec<FieldSpec>> {
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

pub(crate) fn parse_field_attributes(
    attrs: &[Attribute],
    base_accessor: Accessor,
) -> syn::Result<FieldConfig> {
    let mut config = FieldConfig::new(base_accessor);
    for attr in attrs {
        process_datatable_field_attr(attr, &mut config)?;
    }
    Ok(config)
}

fn process_datatable_field_attr(attr: &Attribute, config: &mut FieldConfig) -> syn::Result<()> {
    if !attr.path().is_ident("datatable") {
        return Ok(());
    }
    attr.parse_nested_meta(|meta| process_field_meta_item(&meta, config))
}

fn process_field_meta_item(
    meta: &syn::meta::ParseNestedMeta,
    config: &mut FieldConfig,
) -> syn::Result<()> {
    let Some(ident) = meta.path.get_ident() else {
        return Err(meta.error("unsupported datatable attribute"));
    };
    let ident = ident.to_string();
    match ident.as_str() {
        "column" => handle_column_attribute(meta, config),
        "optional" => handle_optional_attribute(config),
        "default" => handle_default_attribute(meta, config),
        "parse_with" => handle_parse_with_attribute(meta, config),
        "truthy" => handle_truthy_attribute(config),
        "trim" => handle_trim_attribute(config),
        _ => Err(meta.error("unsupported datatable attribute")),
    }
}

fn handle_column_attribute(
    meta: &syn::meta::ParseNestedMeta,
    config: &mut FieldConfig,
) -> syn::Result<()> {
    let value: LitStr = meta.value()?.parse()?;
    config.accessor = Accessor::Column {
        name: value.value(),
    };
    Ok(())
}

#[expect(
    clippy::unnecessary_wraps,
    reason = "Handlers must expose a uniform syn::Result<()> signature."
)]
fn handle_optional_attribute(config: &mut FieldConfig) -> syn::Result<()> {
    config.optional = true;
    Ok(())
}

fn handle_default_attribute(
    meta: &syn::meta::ParseNestedMeta,
    config: &mut FieldConfig,
) -> syn::Result<()> {
    if config.default.is_some() {
        return Err(meta.error("duplicate default attribute"));
    }
    if meta.input.peek(Token![=]) {
        let path: ExprPath = meta.value()?.parse()?;
        config.default = Some(DefaultValue::Function(path));
    } else {
        config.default = Some(DefaultValue::Trait);
    }
    Ok(())
}

fn handle_parse_with_attribute(
    meta: &syn::meta::ParseNestedMeta,
    config: &mut FieldConfig,
) -> syn::Result<()> {
    let path: ExprPath = meta.value()?.parse()?;
    if config.parse_with.replace(path).is_some() {
        return Err(meta.error("duplicate parse_with attribute"));
    }
    Ok(())
}

#[expect(
    clippy::unnecessary_wraps,
    reason = "Handlers must expose a uniform syn::Result<()> signature."
)]
fn handle_truthy_attribute(config: &mut FieldConfig) -> syn::Result<()> {
    config.truthy = true;
    Ok(())
}

#[expect(
    clippy::unnecessary_wraps,
    reason = "Handlers must expose a uniform syn::Result<()> signature."
)]
fn handle_trim_attribute(config: &mut FieldConfig) -> syn::Result<()> {
    config.trim = true;
    Ok(())
}

fn validate_field_config(
    config: &FieldConfig,
    is_option: bool,
    inner_ty: &Type,
    span: Span,
) -> syn::Result<()> {
    ensure_when(
        config.optional && config.default.is_some(),
        span,
        "optional fields cannot specify a default",
    )?;
    ensure_when(
        config.truthy && config.parse_with.is_some(),
        span,
        "truthy and parse_with are mutually exclusive",
    )?;
    ensure_when(
        config.optional && !is_option,
        span,
        "#[datatable(optional)] requires an Option<T> field",
    )?;
    ensure_when(
        is_option && config.default.is_some(),
        span,
        "Option<T> fields cannot define a default value",
    )?;
    ensure_when(
        config.truthy && !is_bool_type(inner_ty),
        span,
        "#[datatable(truthy)] requires a bool field",
    )?;
    Ok(())
}

fn ensure_when(violation: bool, span: Span, message: &str) -> syn::Result<()> {
    if violation {
        Err(syn::Error::new(span, message))
    } else {
        Ok(())
    }
}
