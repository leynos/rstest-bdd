//! Argument extraction and classification helpers for wrapper generation.

/// Fixture argument extracted from a step function.
#[derive(Debug, Clone)]
pub struct FixtureArg {
    pub pat: syn::Ident,
    pub name: syn::Ident,
    pub ty: syn::Type,
}

/// Non-fixture argument extracted from a step function.
#[derive(Debug, Clone)]
pub struct StepArg {
    pub pat: syn::Ident,
    pub ty: syn::Type,
}

/// Represents an argument for a Gherkin data table step function.
///
/// The [`ty`] field stores the Rust type of the argument. This enables
/// type-specific logic such as code generation, validation, or transformation
/// based on the argument's type. Documenting the type here clarifies its role in
/// macro expansion and helps future maintainers understand how type information
/// is propagated.
///
/// # Fields
/// - `pat`: The identifier pattern for the argument.
/// - `ty`: The Rust type of the argument, used for type-specific logic and code generation.
#[derive(Debug, Clone)]
pub struct DataTableArg {
    pub pat: syn::Ident,
    pub ty: syn::Type,
}

/// Gherkin doc string argument extracted from a step function.
#[derive(Debug, Clone)]
pub struct DocStringArg {
    pub pat: syn::Ident,
}

/// Argument ordering as declared in the step function signature.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CallArg {
    Fixture(usize),
    StepArg(usize),
    DataTable,
    DocString,
}

/// Collections of arguments extracted from a step function signature.
#[derive(Clone)]
pub struct ExtractedArgs {
    pub fixtures: Vec<FixtureArg>,
    pub step_args: Vec<StepArg>,
    pub datatable: Option<DataTableArg>,
    pub docstring: Option<DocStringArg>,
    pub call_order: Vec<CallArg>,
}

/// References to extracted arguments for ordered processing.
#[derive(Clone, Copy)]
pub(crate) struct ArgumentCollections<'a> {
    pub(crate) fixtures: &'a [FixtureArg],
    pub(crate) step_args: &'a [StepArg],
    pub(crate) datatable: Option<&'a DataTableArg>,
    pub(crate) docstring: Option<&'a DocStringArg>,
}

impl std::fmt::Debug for ExtractedArgs {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ExtractedArgs")
            .field("fixtures", &self.fixtures.len())
            .field("step_args", &self.step_args.len())
            .field("datatable", &self.datatable.is_some())
            .field("docstring", &self.docstring.is_some())
            .field("call_order", &self.call_order)
            .finish()
    }
}

/// Matches a nested path sequence like `["Vec", "Vec", "String"]` for `Vec<Vec<String>>`.
/// Only the first generic argument at each level is inspected; the final segment may be unparameterised.
fn is_type_seq(ty: &syn::Type, seq: &[&str]) -> bool {
    use syn::{GenericArgument, PathArguments, Type};

    let mut cur = ty;
    for (i, &name) in seq.iter().enumerate() {
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
            _ => {
                if i + 1 != seq.len() {
                    return false;
                }
            }
        }
    }
    true
}

/// Matches a `String` type using [`is_type_seq`].
/// Only the first generic argument at each level is inspected; the final segment may be unparameterised.
fn is_string(ty: &syn::Type) -> bool {
    is_type_seq(ty, &["String"])
}
/// Matches a `Vec<Vec<String>>` type using [`is_type_seq`].
/// Only the first generic argument at each level is inspected; the final segment may be unparameterised.
fn is_datatable(ty: &syn::Type) -> bool {
    is_type_seq(ty, &["Vec", "Vec", "String"])
}
fn should_classify_as_datatable(pat: &syn::Ident, ty: &syn::Type) -> bool {
    pat == "datatable" && is_datatable(ty)
}

/// Removes the `#[datatable]` attribute, returning `true` if present.
///
/// The attribute must be bare (e.g., `#[datatable]`). Any tokens supplied will
/// result in a parse error so callers receive precise diagnostics.
fn extract_datatable_attribute(arg: &mut syn::PatType) -> syn::Result<bool> {
    let mut found = false;
    let mut duplicate = false;
    let mut err_attr: Option<syn::Attribute> = None;
    arg.attrs.retain(|a| {
        if a.path().is_ident("datatable") {
            if found {
                duplicate = true;
            }
            found = true;
            if a.meta.require_path_only().is_err() {
                err_attr = Some(a.clone());
            }
            false
        } else {
            true
        }
    });
    if let Some(attr) = err_attr {
        return Err(syn::Error::new_spanned(
            attr,
            "`#[datatable]` does not take arguments",
        ));
    }
    if duplicate {
        return Err(syn::Error::new_spanned(
            &arg.pat,
            "duplicate `#[datatable]` attribute",
        ));
    }
    Ok(found)
}

/// Validates that a potential datatable argument obeys uniqueness and ordering
/// constraints, returning `true` when classification should proceed.
fn validate_datatable_constraints(
    st: &ExtractedArgs,
    arg: &mut syn::PatType,
    pat: &syn::Ident,
    ty: &syn::Type,
) -> syn::Result<bool> {
    let is_attr = extract_datatable_attribute(arg)?;
    let is_canonical = should_classify_as_datatable(pat, ty);

    if is_attr && pat == "docstring" {
        return Err(syn::Error::new_spanned(
            arg,
            "parameter `docstring` cannot be annotated with #[datatable]",
        ));
    }
    if is_attr || is_canonical {
        if st.datatable.is_some() {
            return Err(syn::Error::new_spanned(
                arg,
                "only one DataTable parameter is permitted",
            ));
        }
        if st.docstring.is_some() {
            return Err(syn::Error::new_spanned(
                arg,
                "DataTable must be declared before DocString to match Gherkin ordering",
            ));
        }
        Ok(true)
    } else if pat == "datatable" {
        Err(syn::Error::new_spanned(
            arg,
            concat!(
                "parameter named `datatable` must have type `Vec<Vec<String>>` ",
                "(or use `#[datatable]` with a type that implements `TryFrom<Vec<Vec<String>>>`)",
            ),
        ))
    } else {
        Ok(false)
    }
}

fn classify_datatable(
    st: &mut ExtractedArgs,
    arg: &mut syn::PatType,
    pat: &syn::Ident,
    ty: &syn::Type,
) -> syn::Result<bool> {
    let has_from = arg.attrs.iter().any(|a| a.path().is_ident("from"));
    if !validate_datatable_constraints(st, arg, pat, ty)? {
        return Ok(false);
    }
    if has_from {
        return Err(syn::Error::new_spanned(
            arg,
            "#[datatable] cannot be combined with #[from]",
        ));
    }
    st.datatable = Some(DataTableArg {
        pat: pat.clone(),
        ty: ty.clone(),
    });
    st.call_order.push(CallArg::DataTable);
    Ok(true)
}
fn is_valid_docstring_arg(st: &ExtractedArgs, pat: &syn::Ident, ty: &syn::Type) -> bool {
    st.docstring.is_none() && pat == "docstring" && is_string(ty)
}

fn classify_docstring(
    st: &mut ExtractedArgs,
    arg: &mut syn::PatType,
    pat: &syn::Ident,
    ty: &syn::Type,
) -> syn::Result<bool> {
    if is_valid_docstring_arg(st, pat, ty) {
        st.docstring = Some(DocStringArg { pat: pat.clone() });
        st.call_order.push(CallArg::DocString);
        Ok(true)
    } else if pat == "docstring" {
        Err(syn::Error::new_spanned(
            arg,
            "only one docstring parameter is permitted and it must have type `String`",
        ))
    } else {
        Ok(false)
    }
}

/// Classifies an argument as either a fixture or a step parameter.
///
/// The function removes any `#[from]` attribute from the argument before
/// classification. Arguments without an explicit `#[from]` attribute are treated
/// as step parameters when their identifier matches a placeholder in the step
/// pattern; otherwise they are classified as fixtures.
fn classify_fixture_or_step(
    st: &mut ExtractedArgs,
    arg: &mut syn::PatType,
    pat: &syn::Ident,
    ty: &syn::Type,
    placeholders: &mut std::collections::HashSet<String>,
) {
    let mut from_name = None;
    arg.attrs.retain(|a| {
        if a.path().is_ident("from") {
            from_name = a.parse_args().ok();
            false
        } else {
            true
        }
    });

    if from_name.is_none() && placeholders.remove(&pat.to_string()) {
        let idx = st.step_args.len();
        st.step_args.push(StepArg {
            pat: pat.clone(),
            ty: ty.clone(),
        });
        st.call_order.push(CallArg::StepArg(idx));
    } else {
        let name = from_name.unwrap_or_else(|| pat.clone());
        let idx = st.fixtures.len();
        st.fixtures.push(FixtureArg {
            pat: pat.clone(),
            name,
            ty: ty.clone(),
        });
        st.call_order.push(CallArg::Fixture(idx));
    }
}

/// Extract fixture, step, data table, and doc string arguments from a function signature.
///
/// # Examples
/// ```rust,ignore
/// use syn::parse_quote;
///
/// let mut func: syn::ItemFn = parse_quote! {
///     fn step(#[from] a: usize, datatable: Vec<Vec<String>>, docstring: String, b: i32) {}
/// };
/// let mut placeholders = std::collections::HashSet::new();
/// placeholders.insert("b".into());
/// let args = extract_args(&mut func, &mut placeholders).unwrap();
/// assert_eq!(args.fixtures.len(), 1);
/// assert_eq!(args.step_args.len(), 1);
/// assert!(args.datatable.is_some());
/// assert!(args.docstring.is_some());
/// assert_eq!(args.call_order.len(), 4);
/// ```
///
/// Note: special arguments must use the canonical names:
/// - data table parameter must be annotated with `#[datatable]` or be named
///   `datatable` and have type `Vec<Vec<String>>`
/// - doc string parameter must be named `docstring` and have type `String`
///
/// At most one `datatable` and one `docstring` parameter are permitted.
// FIXME: https://github.com/leynos/rstest-bdd/issues/54
pub fn extract_args(
    func: &mut syn::ItemFn,
    placeholders: &mut std::collections::HashSet<String>,
) -> syn::Result<ExtractedArgs> {
    let mut state = ExtractedArgs {
        fixtures: vec![],
        step_args: vec![],
        datatable: None,
        docstring: None,
        call_order: vec![],
    };

    for input in &mut func.sig.inputs {
        let syn::FnArg::Typed(arg) = input else {
            return Err(syn::Error::new_spanned(
                input,
                "methods are not supported; remove `self` from step functions",
            ));
        };
        let syn::Pat::Ident(pat_ident) = &*arg.pat else {
            return Err(syn::Error::new_spanned(
                &arg.pat,
                "unsupported parameter pattern; use a simple identifier (e.g., `arg: T`)",
            ));
        };
        let pat = pat_ident.ident.clone();
        let ty = (*arg.ty).clone();
        if classify_datatable(&mut state, arg, &pat, &ty)? {
            continue;
        }
        if classify_docstring(&mut state, arg, &pat, &ty)? {
            continue;
        }
        classify_fixture_or_step(&mut state, arg, &pat, &ty, placeholders);
    }
    if !placeholders.is_empty() {
        let missing = placeholders.iter().cloned().collect::<Vec<_>>().join(", ");
        return Err(syn::Error::new(
            func.sig.ident.span(),
            format!("missing step arguments for placeholders: {missing}"),
        ));
    }
    Ok(state)
}
