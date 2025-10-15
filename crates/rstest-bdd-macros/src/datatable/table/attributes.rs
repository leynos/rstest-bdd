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
        attr.parse_nested_meta(|meta| {
            let Some(ident) = meta.path.get_ident() else {
                return Err(meta.error("unsupported datatable attribute"));
            };
            let key = ident.to_string();
            match key.as_str() {
                "row" => {
                    let ty = parse_row_type(&meta)?;
                    if config.row_ty.replace(ty).is_some() {
                        return Err(meta.error("duplicate row attribute"));
                    }
                }
                "map" | "try_map" => {
                    let path: ExprPath = meta.value()?.parse()?;
                    let kind = if key == "map" {
                        MapKind::Direct(path)
                    } else {
                        MapKind::Try(path)
                    };
                    if config.map.replace(kind).is_some() {
                        return Err(meta.error("duplicate map/try_map attribute"));
                    }
                }
                _ => return Err(meta.error("unsupported datatable attribute")),
            }
            Ok(())
        })?;
    }
    Ok(config)
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
    use super::*;
    use syn::parse_quote;

    #[test]
    fn parse_struct_attrs_supports_row_and_map() {
        let attrs = vec![
            parse_quote!(#[datatable(row = Example)]),
            parse_quote!(#[datatable(map = transform)]),
        ];
        #[expect(clippy::expect_used, reason = "test asserts parsed config")]
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
        #[expect(clippy::expect_used, reason = "test asserts error handling")]
        let err = parse_struct_attrs(&attrs)
            .err()
            .expect("map and try_map together should trigger an error");
        assert!(err.to_string().contains("duplicate map/try_map attribute"));
    }
}
