//! Parsing helpers for `StepArgs` field attributes.

use syn::LitStr;

use super::StepFieldConfig;

/// Dispatch one `step_args` field metadata item to its focused parser.
pub(super) fn process_step_field_meta_item(
    meta: &syn::meta::ParseNestedMeta,
    config: &mut StepFieldConfig,
) -> syn::Result<()> {
    let ident = meta
        .path
        .get_ident()
        .ok_or_else(|| meta.error("unsupported step_args field attribute"))?
        .to_string();

    match ident.as_str() {
        "placeholder" => process_placeholder_attribute(meta, config),
        "trim" => process_trim_attribute(meta, config),
        "parse_with" => process_parse_with_attribute(meta, config),
        _ => Err(meta.error("unsupported step_args field attribute")),
    }
}

/// Parse `placeholder = "name"` for one field.
fn process_placeholder_attribute(
    meta: &syn::meta::ParseNestedMeta,
    config: &mut StepFieldConfig,
) -> syn::Result<()> {
    let value: LitStr = meta.value()?.parse()?;
    if config.placeholder.replace(value.value()).is_some() {
        Err(meta.error("duplicate placeholder attribute"))
    } else {
        Ok(())
    }
}

/// Parse the `trim` flag for one field.
fn process_trim_attribute(
    meta: &syn::meta::ParseNestedMeta,
    config: &mut StepFieldConfig,
) -> syn::Result<()> {
    if config.trim {
        Err(meta.error("duplicate trim attribute"))
    } else {
        config.trim = true;
        Ok(())
    }
}

/// Parse `parse_with = path` for one field.
fn process_parse_with_attribute(
    meta: &syn::meta::ParseNestedMeta,
    config: &mut StepFieldConfig,
) -> syn::Result<()> {
    let parser: syn::ExprPath = meta.value()?.parse()?;
    if config.parse_with.replace(parser).is_some() {
        Err(meta.error("duplicate parse_with attribute"))
    } else {
        Ok(())
    }
}
