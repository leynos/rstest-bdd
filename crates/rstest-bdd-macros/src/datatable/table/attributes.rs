//! Attribute parsing for `#[derive(DataTable)]` derive macros.
//!
//! Extracts and validates `#[datatable(...)]` attributes on structs to configure
//! row types, mapping functions, and fallible transformations.

use syn::{Attribute, ExprPath, LitStr, Type};

pub(crate) enum MapKind {
    Direct(ExprPath),
    Try(ExprPath),
}

pub(crate) struct TableConfig {
    pub(crate) row_ty: Option<Type>,
    pub(crate) map: Option<MapKind>,
}

pub(crate) fn parse_struct_attrs(attrs: &[Attribute]) -> syn::Result<TableConfig> {
    let mut config = TableConfig {
        row_ty: None,
        map: None,
    };
    for attr in attrs
        .iter()
        .filter(|attr| attr.path().is_ident("datatable"))
    {
        attr.parse_nested_meta(|meta| apply_struct_meta(&mut config, &meta))?;
    }
    Ok(config)
}

/// Fold one `#[datatable(..)]` entry into the accumulated table configuration.
fn apply_struct_meta(
    config: &mut TableConfig,
    meta: &syn::meta::ParseNestedMeta<'_>,
) -> syn::Result<()> {
    let Some(ident) = meta.path.get_ident() else {
        return Err(meta.error("unsupported datatable attribute"));
    };
    match ident.to_string().as_str() {
        "row" => set_row_type(config, meta),
        key @ ("map" | "try_map") => set_map(config, meta, key == "map"),
        _ => Err(meta.error("unsupported datatable attribute")),
    }
}

fn set_row_type(
    config: &mut TableConfig,
    meta: &syn::meta::ParseNestedMeta<'_>,
) -> syn::Result<()> {
    let ty = parse_row_type(meta)?;
    if config.row_ty.replace(ty).is_some() {
        return Err(meta.error("duplicate row attribute"));
    }
    Ok(())
}

fn set_map(
    config: &mut TableConfig,
    meta: &syn::meta::ParseNestedMeta<'_>,
    is_direct: bool,
) -> syn::Result<()> {
    let path: ExprPath = meta.value()?.parse()?;
    let kind = if is_direct {
        MapKind::Direct(path)
    } else {
        MapKind::Try(path)
    };
    if config.map.replace(kind).is_some() {
        return Err(meta.error("duplicate map/try_map attribute"));
    }
    Ok(())
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

#[cfg(test)]
mod tests {
    //! Unit tests for `#[datatable]` table attribute parsing.

    use syn::parse_quote;

    use super::*;

    #[test]
    fn parse_struct_attrs_supports_row_and_map() {
        let attrs = vec![
            parse_quote!(#[datatable(row = Example)]),
            parse_quote!(#[datatable(map = transform)]),
        ];
        let config = parse_struct_attrs(&attrs).expect("failed to parse struct attrs");
        assert!(config.row_ty.is_some());
        assert!(matches!(config.map, Some(MapKind::Direct(_))));
    }

    #[test]
    fn parse_struct_attrs_rejects_conflicting_map_variants() {
        let attrs = vec![
            parse_quote!(#[datatable(map = transform)]),
            parse_quote!(#[datatable(try_map = fallible_transform)]),
        ];
        let err = parse_struct_attrs(&attrs)
            .err()
            .expect("map and try_map together should trigger an error");
        assert!(err.to_string().contains("duplicate map/try_map attribute"));
    }
}
