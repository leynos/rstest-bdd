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

/// Collections of arguments extracted from a step function signature.
pub(crate) struct ExtractedArgs {
    pub(crate) fixtures: Vec<FixtureArg>,
    pub(crate) step_args: Vec<StepArg>,
    pub(crate) datatable: Option<DataTableArg>,
    pub(crate) docstring: Option<DocStringArg>,
    pub(crate) call_order: Vec<CallArg>,
}

type Classifier =
    fn(&mut ExtractedArgs, &mut syn::PatType, syn::Ident, syn::Type) -> syn::Result<bool>;

fn is_type_seq(ty: &syn::Type, seq: &[&str]) -> bool {
    use syn::{GenericArgument, PathArguments, Type};

    let mut cur = ty;
    for &name in seq {
        let Type::Path(tp) = cur else { return false };
        let Some(segment) = tp.path.segments.last() else {
            return false;
        };
        if segment.ident != name {
            return false;
        }
        match &segment.arguments {
            PathArguments::AngleBracketed(ab) if !ab.args.is_empty() => {
                if let Some(GenericArgument::Type(inner)) = ab.args.get(0) {
                    cur = inner;
                    continue;
                }
                return false;
            }
            _ => {}
        }
    }
    true
}

fn is_string(ty: &syn::Type) -> bool {
    is_type_seq(ty, &["String"])
}
fn is_datatable(ty: &syn::Type) -> bool {
    is_type_seq(ty, &["Vec", "Vec", "String"])
}

#[expect(clippy::unnecessary_wraps, reason = "conforms to classifier signature")]
fn classify_fixture(
    st: &mut ExtractedArgs,
    arg: &mut syn::PatType,
    pat: syn::Ident,
    ty: syn::Type,
) -> syn::Result<bool> {
    let mut name = None;
    arg.attrs.retain(|a| {
        if a.path().is_ident("from") {
            name = a.parse_args().ok();
            false
        } else {
            true
        }
    });
    if let Some(name) = name {
        let idx = st.fixtures.len();
        st.fixtures.push(FixtureArg { pat, name, ty });
        st.call_order.push(CallArg::Fixture(idx));
        Ok(true)
    } else {
        Ok(false)
    }
}

#[expect(
    clippy::needless_pass_by_value,
    reason = "uniform classifier signature"
)]
fn classify_datatable(
    st: &mut ExtractedArgs,
    arg: &mut syn::PatType,
    pat: syn::Ident,
    ty: syn::Type,
) -> syn::Result<bool> {
    if st.datatable.is_none() && pat == "datatable" && is_datatable(&ty) {
        if st.docstring.is_some() {
            return Err(syn::Error::new_spanned(
                arg,
                "datatable must be declared before docstring to match Gherkin ordering",
            ));
        }
        st.datatable = Some(DataTableArg { pat });
        st.call_order.push(CallArg::DataTable);
        Ok(true)
    } else {
        Ok(false)
    }
}

#[expect(clippy::unnecessary_wraps, reason = "conforms to classifier signature")]
#[expect(
    clippy::needless_pass_by_value,
    reason = "uniform classifier signature"
)]
fn classify_docstring(
    st: &mut ExtractedArgs,
    _arg: &mut syn::PatType,
    pat: syn::Ident,
    ty: syn::Type,
) -> syn::Result<bool> {
    if st.docstring.is_none() && pat == "docstring" && is_string(&ty) {
        st.docstring = Some(DocStringArg { pat });
        st.call_order.push(CallArg::DocString);
        Ok(true)
    } else {
        Ok(false)
    }
}

#[expect(clippy::unnecessary_wraps, reason = "conforms to classifier signature")]
fn classify_step_arg(
    st: &mut ExtractedArgs,
    _arg: &mut syn::PatType,
    pat: syn::Ident,
    ty: syn::Type,
) -> syn::Result<bool> {
    let idx = st.step_args.len();
    st.step_args.push(StepArg { pat, ty });
    st.call_order.push(CallArg::StepArg(idx));
    Ok(true)
}

const CLASSIFIERS: &[Classifier] = &[
    classify_fixture,
    classify_datatable,
    classify_docstring,
    classify_step_arg,
];

/// Extract fixture, step, and special arguments from a function signature.
///
/// # Examples
/// ```rust,ignore
/// use syn::parse_quote;
///
/// let mut func: syn::ItemFn = parse_quote! {
///     fn step(#[from] a: usize, datatable: Vec<Vec<String>>, docstring: String, b: i32) {}
/// };
/// let args = extract_args(&mut func).unwrap();
/// assert_eq!(args.fixtures.len(), 1);
/// assert_eq!(args.step_args.len(), 1);
/// assert!(args.datatable.is_some());
/// assert!(args.docstring.is_some());
/// assert_eq!(args.call_order.len(), 4);
/// ```
// FIXME: https://github.com/leynos/rstest-bdd/issues/54
pub(crate) fn extract_args(func: &mut syn::ItemFn) -> syn::Result<ExtractedArgs> {
    let mut state = ExtractedArgs {
        fixtures: vec![],
        step_args: vec![],
        datatable: None,
        docstring: None,
        call_order: vec![],
    };

    for input in &mut func.sig.inputs {
        let syn::FnArg::Typed(arg) = input else {
            return Err(syn::Error::new_spanned(input, "methods not supported"));
        };
        let syn::Pat::Ident(pat_ident) = &*arg.pat else {
            return Err(syn::Error::new_spanned(&arg.pat, "unsupported pattern"));
        };
        let pat = pat_ident.ident.clone();
        let ty = (*arg.ty).clone();

        for class in CLASSIFIERS {
            if class(&mut state, arg, pat.clone(), ty.clone())? {
                break;
            }
        }
    }

    Ok(state)
}
