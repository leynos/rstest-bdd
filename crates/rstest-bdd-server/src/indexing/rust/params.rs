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

    let segments: Vec<&syn::Ident> = type_path.path.segments.iter().map(|s| &s.ident).collect();
    match segments.as_slice() {
        [only] => *only == "String",
        [first, second, third] => {
            (*first == "std" || *first == "alloc") && *second == "string" && *third == "String"
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    //! Unit tests for indexed step-function parameter classification.

    use rstest::rstest;

    use super::{
        param_name, parameter_is_datatable, parameter_is_docstring, parameter_is_step_struct,
    };

    fn typed_parameter(arg: syn::FnArg) -> syn::PatType {
        match arg {
            syn::FnArg::Typed(pat_type) => pat_type,
            syn::FnArg::Receiver(_) => panic!("test parameter should be a typed argument"),
        }
    }

    #[test]
    fn param_name_ignores_non_identifier_patterns() {
        let pat_type = typed_parameter(syn::parse_quote!((left, right): (u32, u32)));
        assert_eq!(param_name(&pat_type.pat), None);
    }

    #[rstest]
    #[case::attribute(syn::parse_quote!(#[datatable] rows: Vec<Vec<String>>), true)]
    #[case::name(syn::parse_quote!(datatable: Vec<Vec<String>>), true)]
    #[case::plain(syn::parse_quote!(count: u32), false)]
    fn datatable_parameters_are_detected(#[case] arg: syn::FnArg, #[case] expected: bool) {
        let pat_type = typed_parameter(arg);
        let name = param_name(&pat_type.pat);
        assert_eq!(parameter_is_datatable(&pat_type, name.as_deref()), expected);
    }

    #[rstest]
    #[case::attribute(syn::parse_quote!(#[step_args] args: LoginArgs), true)]
    #[case::plain(syn::parse_quote!(args: LoginArgs), false)]
    fn step_struct_parameters_are_detected(#[case] arg: syn::FnArg, #[case] expected: bool) {
        assert_eq!(parameter_is_step_struct(&typed_parameter(arg)), expected);
    }

    #[rstest]
    #[case::bare_string(syn::parse_quote!(docstring: String), true)]
    #[case::std_path(syn::parse_quote!(docstring: std::string::String), true)]
    #[case::alloc_path(syn::parse_quote!(docstring: alloc::string::String), true)]
    #[case::wrong_type(syn::parse_quote!(docstring: u32), false)]
    #[case::wrong_name(syn::parse_quote!(body: String), false)]
    #[case::reference(syn::parse_quote!(docstring: &str), false)]
    #[case::two_segments(syn::parse_quote!(docstring: string::String), false)]
    fn docstring_parameters_require_a_string_type(#[case] arg: syn::FnArg, #[case] expected: bool) {
        let pat_type = typed_parameter(arg);
        let name = param_name(&pat_type.pat);
        assert_eq!(
            parameter_is_docstring(name.as_deref(), &pat_type.ty),
            expected
        );
    }
}
