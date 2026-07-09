//! Parameter classification for indexed Rust step functions.
//!
//! Mirrors the macro behaviour: a data table is expected when a parameter is
//! named `datatable` or carries `#[datatable]`; a doc string is expected when
//! a parameter named `docstring` has a `String` type; `#[step_args]` marks a
//! bundled step-struct parameter.

pub(super) fn param_name(pat: &syn::Pat) -> Option<String> {
    match pat {
        syn::Pat::Ident(pat_ident) => Some(pat_ident.ident.to_string()),
        _ => None,
    }
}

pub(super) fn parameter_is_datatable(pat_type: &syn::PatType, name: Option<&str>) -> bool {
    if name.is_some_and(|value| value == "datatable") {
        return true;
    }

    pat_type.attrs.iter().any(|attr| {
        attr.path()
            .segments
            .last()
            .is_some_and(|seg| seg.ident == "datatable")
    })
}

/// Check if a parameter has the `#[step_args]` attribute.
///
/// Step struct parameters bundle all placeholders into a single struct,
/// so they should be counted as step arguments regardless of their name.
pub(super) fn parameter_is_step_struct(pat_type: &syn::PatType) -> bool {
    pat_type.attrs.iter().any(|attr| {
        attr.path()
            .segments
            .last()
            .is_some_and(|seg| seg.ident == "step_args")
    })
}

pub(super) fn parameter_is_docstring(name: Option<&str>, ty: &syn::Type) -> bool {
    if name.is_none_or(|value| value != "docstring") {
        return false;
    }
    type_is_string(ty)
}

fn type_is_string(ty: &syn::Type) -> bool {
    let syn::Type::Path(type_path) = ty else {
        return false;
    };

    let mut segments = type_path.path.segments.iter();
    let Some(first) = segments.next() else {
        return false;
    };
    let Some(second) = segments.next() else {
        return first.ident == "String";
    };
    let Some(third) = segments.next() else {
        return false;
    };
    if segments.next().is_some() {
        return false;
    }

    (first.ident == "std" || first.ident == "alloc")
        && second.ident == "string"
        && third.ident == "String"
}
