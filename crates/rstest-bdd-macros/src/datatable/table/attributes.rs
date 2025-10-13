use syn::{Attribute, ExprPath, LitStr, Type};

pub(crate) struct TableConfig {
    pub(crate) row_ty: Option<Type>,
    pub(crate) map: Option<ExprPath>,
    pub(crate) try_map: Option<ExprPath>,
}

pub(crate) fn parse_struct_attrs(attrs: &[Attribute]) -> syn::Result<TableConfig> {
    let mut config = TableConfig {
        row_ty: None,
        map: None,
        try_map: None,
    };
    for attr in attrs
        .iter()
        .filter(|attr| attr.path().is_ident("datatable"))
    {
        attr.parse_nested_meta(|meta| process_meta_item(&meta, &mut config))?;
    }
    Ok(config)
}

fn process_meta_item(
    meta: &syn::meta::ParseNestedMeta,
    config: &mut TableConfig,
) -> syn::Result<()> {
    if meta.path.is_ident("row") {
        handle_row_attribute(meta, config)
    } else if meta.path.is_ident("map") {
        handle_map_attribute(meta, config)
    } else if meta.path.is_ident("try_map") {
        handle_try_map_attribute(meta, config)
    } else {
        Err(meta.error("unsupported datatable attribute"))
    }
}

#[expect(
    clippy::needless_pass_by_value,
    reason = "consume Option returned from Option::replace when guarding duplicates"
)]
fn ensure_no_duplicate<T>(
    result: Option<T>,
    meta: &syn::meta::ParseNestedMeta,
    message: &str,
) -> syn::Result<()> {
    match result {
        Some(_) => Err(meta.error(message)),
        None => Ok(()),
    }
}

macro_rules! handle_attribute_with_duplicate_check {
    ($meta:expr, $field:expr, $parser:expr, $attr_name:literal $(,)?) => {{
        let value = $parser?;
        ensure_no_duplicate(
            $field.replace(value),
            $meta,
            concat!("duplicate ", $attr_name, " attribute"),
        )
    }};
}

fn handle_row_attribute(
    meta: &syn::meta::ParseNestedMeta,
    config: &mut TableConfig,
) -> syn::Result<()> {
    handle_attribute_with_duplicate_check!(meta, config.row_ty, parse_row_type(meta), "row")
}

fn handle_map_attribute(
    meta: &syn::meta::ParseNestedMeta,
    config: &mut TableConfig,
) -> syn::Result<()> {
    handle_attribute_with_duplicate_check!(
        meta,
        config.map,
        meta.value()?.parse::<ExprPath>(),
        "map",
    )
}

fn handle_try_map_attribute(
    meta: &syn::meta::ParseNestedMeta,
    config: &mut TableConfig,
) -> syn::Result<()> {
    handle_attribute_with_duplicate_check!(
        meta,
        config.try_map,
        meta.value()?.parse::<ExprPath>(),
        "try_map",
    )
}

fn parse_row_type(meta: &syn::meta::ParseNestedMeta) -> syn::Result<Type> {
    let value = meta.value()?;
    if value.peek(LitStr) {
        let lit: LitStr = value.parse()?;
        syn::parse_str(&lit.value())
    } else {
        value.parse()
    }
}
