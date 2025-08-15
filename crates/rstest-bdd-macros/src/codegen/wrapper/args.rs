//! Argument extraction and classification helpers for wrapper generation.

/// Fixture argument extracted from a step function.
pub(crate) struct FixtureArg {
    pub(crate) pat: syn::Ident,
    pub(crate) name: syn::Ident,
    pub(crate) ty: syn::Type,
}

/// Non-fixture argument extracted from a step function.
pub(crate) struct StepArg {
    pub(crate) pat: syn::Ident,
    pub(crate) ty: syn::Type,
}

/// Data table argument extracted from a step function.
pub(crate) struct DataTableArg {
    pub(crate) pat: syn::Ident,
}

/// Doc string argument extracted from a step function.
pub(crate) struct DocStringArg {
    pub(crate) pat: syn::Ident,
}

/// Argument ordering as declared in the step function signature.
pub(crate) enum CallArg {
    Fixture(usize),
    StepArg(usize),
    DataTable,
    DocString,
}

/// Extract fixture and step arguments from a function signature.
#[expect(clippy::type_complexity, reason = "return type defined by API")]
// FIXME: https://github.com/leynos/rstest-bdd/issues/54
pub(crate) fn extract_args(
    func: &mut syn::ItemFn,
) -> syn::Result<(
    Vec<FixtureArg>,
    Vec<StepArg>,
    Option<DataTableArg>,
    Option<DocStringArg>,
    Vec<CallArg>,
)> {
    let mut fixtures = Vec::new();
    let mut step_args = Vec::new();
    let mut datatable = None;
    let mut docstring = None;
    let mut call_order = Vec::new();

    for input in &mut func.sig.inputs {
        let syn::FnArg::Typed(arg) = input else {
            return Err(syn::Error::new_spanned(input, "methods not supported"));
        };

        let mut fixture_name = None;
        arg.attrs.retain(|a| {
            if a.path().is_ident("from") {
                fixture_name = a.parse_args::<syn::Ident>().ok();
                false
            } else {
                true
            }
        });

        let pat = match &*arg.pat {
            syn::Pat::Ident(i) => i.ident.clone(),
            _ => {
                return Err(syn::Error::new_spanned(&arg.pat, "unsupported pattern"));
            }
        };

        let ty = (*arg.ty).clone();

        if let Some(name) = fixture_name {
            let idx = fixtures.len();
            fixtures.push(FixtureArg { pat, name, ty });
            call_order.push(CallArg::Fixture(idx));
        } else if is_datatable_arg(&datatable, &pat, &ty) {
            if docstring.is_some() {
                // Gherkin places data tables before doc strings, so require the
                // same order in step signatures to avoid reordering arguments.
                return Err(syn::Error::new_spanned(
                    &arg.pat,
                    "datatable must be declared before docstring to match Gherkin ordering",
                ));
            }
            datatable = Some(DataTableArg { pat });
            call_order.push(CallArg::DataTable);
        } else if is_docstring_arg(&docstring, &pat, &ty) {
            docstring = Some(DocStringArg { pat });
            call_order.push(CallArg::DocString);
        } else {
            let idx = step_args.len();
            step_args.push(StepArg { pat, ty });
            call_order.push(CallArg::StepArg(idx));
        }
    }

    Ok((fixtures, step_args, datatable, docstring, call_order))
}

fn is_vec_vec_string(ty: &syn::Type) -> bool {
    is_vec_of(ty, |inner| is_vec_of(inner, is_string_type))
}

fn is_vec_of<F>(ty: &syn::Type, check_inner: F) -> bool
where
    F: FnOnce(&syn::Type) -> bool,
{
    use syn::{GenericArgument, PathArguments, Type};

    let Type::Path(tp) = ty else { return false };
    let Some(seg) = tp.path.segments.last() else {
        return false;
    };
    if seg.ident != "Vec" {
        return false;
    }
    let PathArguments::AngleBracketed(args) = &seg.arguments else {
        return false;
    };
    let Some(GenericArgument::Type(inner_ty)) = args.args.first() else {
        return false;
    };

    check_inner(inner_ty)
}

fn is_string_type(ty: &syn::Type) -> bool {
    matches!(
        ty,
        syn::Type::Path(tp) if tp.path.segments.last().is_some_and(|seg| seg.ident == "String")
    )
}

/// Determines if a function parameter should be treated as a docstring argument.
///
/// Detection relies on the parameter being named `docstring` and having the
/// concrete type `String`. Renaming the parameter or using a type alias will
/// prevent detection.
///
/// # Examples
/// ```rust,ignore
/// # use proc_macro2::Span;
/// # use syn::{parse_quote, Ident};
/// let ty: syn::Type = parse_quote! { String };
/// let name = Ident::new("docstring", Span::call_site());
/// let none = None;
/// assert!(is_docstring_arg(&none, &name, &ty));
/// ```
#[expect(clippy::ref_option, reason = "signature defined by requirements")]
// FIXME: https://github.com/leynos/rstest-bdd/issues/54
fn is_docstring_arg(
    existing_docstring: &Option<DocStringArg>,
    param_name: &syn::Ident,
    param_type: &syn::Type,
) -> bool {
    existing_docstring.is_none() && param_name == "docstring" && is_string_type(param_type)
}

/// Determines if a function parameter should be treated as a datatable argument.
///
/// Detection relies on the parameter being named `datatable` and having the
/// concrete type `Vec<Vec<String>>`. Renaming the parameter or using a type alias
/// will prevent detection.
///
/// # Examples
/// ```rust,ignore
/// # use proc_macro2::Span;
/// # use syn::{parse_quote, Ident};
/// let ty: syn::Type = parse_quote! { Vec<Vec<String>> };
/// let name = Ident::new("datatable", Span::call_site());
/// let none = None;
/// assert!(is_datatable_arg(&none, &name, &ty));
/// ```
#[expect(clippy::ref_option, reason = "signature defined by requirements")]
// FIXME: https://github.com/leynos/rstest-bdd/issues/54
fn is_datatable_arg(
    existing_datatable: &Option<DataTableArg>,
    param_name: &syn::Ident,
    param_type: &syn::Type,
) -> bool {
    existing_datatable.is_none() && param_name == "datatable" && is_vec_vec_string(param_type)
}
