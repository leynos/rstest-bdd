use syn::{spanned::Spanned, GenericArgument, PathArguments, Type, TypePath};

pub(crate) fn option_inner_type(ty: &Type) -> syn::Result<(bool, Type)> {
    let Type::Path(TypePath { path, .. }) = ty else {
        return Ok((false, ty.clone()));
    };
    let Some(segment) = path.segments.last() else {
        return Ok((false, ty.clone()));
    };
    if segment.ident != "Option" {
        return Ok((false, ty.clone()));
    }
    let PathArguments::AngleBracketed(args) = &segment.arguments else {
        return Err(syn::Error::new(
            ty.span(),
            "Option<T> must specify an inner type",
        ));
    };
    let Some(GenericArgument::Type(inner)) = args.args.first() else {
        return Err(syn::Error::new(
            ty.span(),
            "Option<T> must specify an inner type",
        ));
    };
    Ok((true, inner.clone()))
}

pub(crate) fn is_string_type(ty: &Type) -> bool {
    if let Type::Path(path) = ty {
        if let Some(segment) = path.path.segments.last() {
            return segment.ident == "String" && matches!(segment.arguments, PathArguments::None);
        }
    }
    false
}

pub(crate) fn is_bool_type(ty: &Type) -> bool {
    if let Type::Path(path) = ty {
        if let Some(segment) = path.path.segments.last() {
            return segment.ident == "bool" && matches!(segment.arguments, PathArguments::None);
        }
    }
    false
}
