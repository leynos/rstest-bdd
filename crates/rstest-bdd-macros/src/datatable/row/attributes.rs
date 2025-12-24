//! Attribute parsing and field specification for `#[derive(DataTableRow)]`.
//!
//! Parses struct-level and field-level `#[datatable(...)]` attributes,
//! validates field configurations (optional/default/truthy), and produces
//! field specifications consumed by the bindings module.

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
    let ident = meta
        .path
        .get_ident()
        .ok_or_else(|| meta.error("unsupported datatable attribute"))?
        .to_string();

    match ident.as_str() {
        "optional" | "truthy" | "trim" => process_flag_attribute(meta, ident.as_str(), config),
        "column" => process_column_attribute(meta, config),
        "default" => process_default_attribute(meta, config),
        "parse_with" => process_parse_with_attribute(meta, config),
        _ => Err(meta.error("unsupported datatable attribute")),
    }
}

fn process_flag_attribute(
    meta: &syn::meta::ParseNestedMeta,
    ident: &str,
    config: &mut FieldConfig,
) -> syn::Result<()> {
    if meta.input.peek(Token![=]) {
        return Err(meta.error(format!("`{ident}` takes no value")));
    }
    match ident {
        "optional" => config.optional = true,
        "truthy" => config.truthy = true,
        "trim" => config.trim = true,
        _ => unreachable!("handled in caller match"),
    }
    Ok(())
}

fn process_column_attribute(
    meta: &syn::meta::ParseNestedMeta,
    config: &mut FieldConfig,
) -> syn::Result<()> {
    let value: LitStr = meta.value()?.parse()?;
    config.accessor = Accessor::Column {
        name: value.value(),
    };
    Ok(())
}

fn process_default_attribute(
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

fn process_parse_with_attribute(
    meta: &syn::meta::ParseNestedMeta,
    config: &mut FieldConfig,
) -> syn::Result<()> {
    let path: ExprPath = meta.value()?.parse()?;
    if config.parse_with.replace(path).is_some() {
        Err(meta.error("duplicate parse_with attribute"))
    } else {
        Ok(())
    }
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
        "optional fields already default to None; remove the `default` attribute",
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

#[cfg(test)]
mod tests {
    use super::*;
    use syn::{Data, parse_quote};

    #[test]
    fn parse_struct_config_reads_rename_rule() {
        let input: syn::DeriveInput = parse_quote! {
            #[derive(DataTableRow)]
            #[datatable(rename_all = "Title Case")]
            struct Example {
                value: String,
            }
        };
        #[expect(clippy::expect_used, reason = "test asserts parsed config")]
        let config = parse_struct_config(&input.attrs).expect("failed to parse struct config");
        assert!(matches!(config.rename_rule, Some(RenameRule::Title)));
    }

    #[test]
    fn parse_field_attributes_sets_flags() {
        let field: syn::Field = parse_quote! {
            #[datatable(optional, truthy, trim)]
            flag: Option<bool>
        };
        let base = Accessor::Column {
            name: String::from("flag"),
        };
        #[expect(clippy::expect_used, reason = "test asserts parsed config")]
        let config =
            parse_field_attributes(&field.attrs, base).expect("failed to parse field attributes");
        assert!(config.optional);
        assert!(config.truthy);
        assert!(config.trim);
    }

    #[test]
    fn parse_field_attributes_rejects_duplicate_defaults() {
        let field: syn::Field = parse_quote! {
            #[datatable(default, default = provide)]
            value: usize
        };
        let base = Accessor::Column {
            name: String::from("value"),
        };
        #[expect(clippy::expect_used, reason = "test asserts error handling")]
        let err = parse_field_attributes(&field.attrs, base)
            .err()
            .expect("duplicate default must error");
        assert!(err.to_string().contains("duplicate default attribute"));
    }

    #[test]
    fn collect_fields_rejects_optional_without_option() {
        let input: syn::DeriveInput = parse_quote! {
            struct Example {
                #[datatable(optional)]
                flag: bool,
            }
        };
        #[expect(clippy::expect_used, reason = "test asserts parsed config")]
        let config = parse_struct_config(&input.attrs).expect("failed to parse struct config");
        let Data::Struct(data) = &input.data else {
            unreachable!("test input must be a struct");
        };
        let fields = &data.fields;
        #[expect(clippy::expect_used, reason = "test asserts error handling")]
        let err = collect_fields(fields, &config)
            .err()
            .expect("optional on non-Option should error");
        assert!(
            err.to_string()
                .contains("#[datatable(optional)] requires an Option<T> field")
        );
    }

    #[test]
    fn collect_fields_rejects_truthy_on_non_bool() {
        let input: syn::DeriveInput = parse_quote! {
            struct Example {
                #[datatable(truthy)]
                value: String,
            }
        };
        #[expect(clippy::expect_used, reason = "test asserts parsed config")]
        let config = parse_struct_config(&input.attrs).expect("failed to parse struct config");
        let Data::Struct(data) = &input.data else {
            unreachable!("test input must be a struct");
        };
        let fields = &data.fields;
        #[expect(clippy::expect_used, reason = "test asserts error handling")]
        let err = collect_fields(fields, &config)
            .err()
            .expect("truthy on non-bool should error");
        assert!(
            err.to_string()
                .contains("#[datatable(truthy)] requires a bool field")
        );
    }
}
