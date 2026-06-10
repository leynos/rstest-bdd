//! Function-parameter parsing for Rust step indexing.
//!
//! Converts `syn` function signatures into [`IndexedStepParameter`] records,
//! classifying each parameter as a data table, doc string, or step-struct
//! argument using the same conventions as the `rstest-bdd` macros. Type
//! rendering is delegated to the sibling [`type_render`](super::type_render)
//! module.

use super::IndexedStepParameter;
use super::type_render;

/// Parse function parameters into indexed step parameters.
pub(super) fn parse_function_parameters(
    sig_inputs: &syn::punctuated::Punctuated<syn::FnArg, syn::token::Comma>,
) -> Vec<IndexedStepParameter> {
    sig_inputs
        .iter()
        .map(|input| match input {
            syn::FnArg::Receiver(_) => IndexedStepParameter {
                name: Some("self".to_string()),
                ty: "Self".to_string(),
                is_datatable: false,
                is_docstring: false,
                is_step_struct: false,
            },
            syn::FnArg::Typed(pat_type) => {
                let name = param_name(&pat_type.pat);
                let ty = type_render::render_type(&pat_type.ty);
                let is_datatable = parameter_is_datatable(pat_type, name.as_deref());
                let is_docstring = parameter_is_docstring(name.as_deref(), &pat_type.ty);
                let is_step_struct = parameter_is_step_struct(pat_type);
                IndexedStepParameter {
                    name,
                    ty,
                    is_datatable,
                    is_docstring,
                    is_step_struct,
                }
            }
        })
        .collect()
}

fn param_name(pat: &syn::Pat) -> Option<String> {
    match pat {
        syn::Pat::Ident(pat_ident) => Some(pat_ident.ident.to_string()),
        _ => None,
    }
}

fn parameter_is_datatable(pat_type: &syn::PatType, name: Option<&str>) -> bool {
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
fn parameter_is_step_struct(pat_type: &syn::PatType) -> bool {
    pat_type.attrs.iter().any(|attr| {
        attr.path()
            .segments
            .last()
            .is_some_and(|seg| seg.ident == "step_args")
    })
}

fn parameter_is_docstring(name: Option<&str>, ty: &syn::Type) -> bool {
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
