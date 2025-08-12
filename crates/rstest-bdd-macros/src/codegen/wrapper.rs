//! Generation of wrapper functions for step definitions.

use super::keyword_to_token;
use proc_macro2::TokenStream as TokenStream2;
use quote::{format_ident, quote};
use std::sync::atomic::{AtomicUsize, Ordering};

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
)> {
    let mut fixtures = Vec::new();
    let mut step_args = Vec::new();
    let mut datatable = None;
    let mut docstring = None;

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
            fixtures.push(FixtureArg { pat, name, ty });
        } else if is_datatable_arg(&datatable, &pat, &ty) {
            if docstring.is_some() {
                return Err(syn::Error::new_spanned(
                    &arg.pat,
                    "datatable must be declared before docstring",
                ));
            }
            datatable = Some(DataTableArg { pat });
        } else if is_docstring_arg(&docstring, &pat, &ty) {
            docstring = Some(DocStringArg { pat });
        } else {
            step_args.push(StepArg { pat, ty });
        }
    }

    Ok((fixtures, step_args, datatable, docstring))
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

/// Generate declaration for a data table argument.
fn gen_datatable_decl(
    datatable: Option<&DataTableArg>,
    pattern: &syn::LitStr,
) -> Option<TokenStream2> {
    datatable.map(|DataTableArg { pat }| {
        quote! {
            let #pat: Vec<Vec<String>> = _table
                .ok_or_else(|| format!("Step '{}' requires a data table", #pattern))?
                .iter()
                .map(|row| row.iter().map(|cell| cell.to_string()).collect())
                .collect();
        }
    })
}

/// Generate declaration for a doc string argument.
fn gen_docstring_decl(
    docstring: Option<&DocStringArg>,
    pattern: &syn::LitStr,
) -> Option<TokenStream2> {
    docstring.map(|DocStringArg { pat }| {
        quote! {
            let #pat: String = _docstring
                .ok_or_else(|| format!("Step '{}' requires a doc string", #pattern))?
                .to_string();
        }
    })
}

/// Configuration required to generate a wrapper.
pub(crate) struct WrapperConfig<'a> {
    pub(crate) ident: &'a syn::Ident,
    pub(crate) fixtures: &'a [FixtureArg],
    pub(crate) step_args: &'a [StepArg],
    pub(crate) datatable: Option<&'a DataTableArg>,
    pub(crate) docstring: Option<&'a DocStringArg>,
    pub(crate) pattern: &'a syn::LitStr,
    pub(crate) keyword: rstest_bdd::StepKeyword,
}

/// Generate declarations for fixture values.
///
/// Non-reference fixtures must implement [`Clone`] because wrappers clone
/// them to hand ownership to the step function.
fn gen_fixture_decls(fixtures: &[FixtureArg], ident: &syn::Ident) -> Vec<TokenStream2> {
    fixtures
        .iter()
        .map(|FixtureArg { pat, name, ty }| {
            let lookup_ty = if let syn::Type::Reference(r) = ty {
                &*r.elem
            } else {
                ty
            };
            let clone_suffix = if matches!(ty, syn::Type::Reference(_)) {
                quote! {}
            } else {
                quote! { .cloned() }
            };
            quote! {
                let #pat: #ty = ctx
                    .get::<#lookup_ty>(stringify!(#name))
                    #clone_suffix
                    .ok_or_else(|| format!(
                        "Missing fixture '{}' of type '{}' for step function '{}'",
                        stringify!(#name),
                        stringify!(#lookup_ty),
                        stringify!(#ident)
                    ))?;
            }
        })
        .collect()
}

/// Generate code to parse step arguments from regex captures.
fn gen_step_parses(step_args: &[StepArg], captured: &[TokenStream2]) -> Vec<TokenStream2> {
    step_args
        .iter()
        .zip(captured.iter())
        .map(|(StepArg { pat, ty }, capture)| {
            quote! {
                let #pat: #ty = (#capture)
                    .parse()
                    .unwrap_or_else(|_| {
                        panic!(
                            "failed to parse argument '{}' of type '{}' from '{}' with captured value: '{:?}'",
                            stringify!(#pat),
                            stringify!(#ty),
                            #capture,
                            #capture
                        )
                    });
            }
        })
        .collect()
}

static COUNTER: AtomicUsize = AtomicUsize::new(0);

/// Generate unique identifiers for the wrapper components.
///
/// Returns identifiers for the wrapper function, fixture array constant, and
/// pattern constant.
///
/// # Examples
/// ```rust,ignore
/// # use syn::Ident;
/// # use proc_macro2::Span;
/// let ident = Ident::new("step_fn", Span::call_site());
/// let (w, c, p) = generate_wrapper_identifiers(&ident, 1);
/// assert!(w.to_string().contains("step_fn"));
/// ```
fn generate_wrapper_identifiers(
    ident: &syn::Ident,
    id: usize,
) -> (proc_macro2::Ident, proc_macro2::Ident, proc_macro2::Ident) {
    let wrapper_ident = format_ident!("__rstest_bdd_wrapper_{}_{}", ident, id);
    let ident_upper = ident.to_string().to_uppercase();
    let const_ident = format_ident!("__RSTEST_BDD_FIXTURES_{}_{}", ident_upper, id);
    let pattern_ident = format_ident!("__RSTEST_BDD_PATTERN_{}_{}", ident_upper, id);
    (wrapper_ident, const_ident, pattern_ident)
}

/// Generate the wrapper function body and pattern constant.
///
/// # Examples
/// ```rust,ignore
/// # use syn::parse_quote;
/// # use proc_macro2::Ident;
/// # let ident = Ident::new("step", proc_macro2::Span::call_site());
/// # let config = WrapperConfig { ident: &ident, fixtures: &[], step_args: &[], datatable: None, pattern: &parse_quote!(""), keyword: rstest_bdd::StepKeyword::Given };
/// # let (wrapper_ident, _, pattern_ident) = generate_wrapper_identifiers(config.ident, 0);
/// let tokens = generate_wrapper_body(&config, &wrapper_ident, &pattern_ident);
/// assert!(tokens.to_string().contains("fn"));
/// ```
fn generate_wrapper_body(
    config: &WrapperConfig<'_>,
    wrapper_ident: &proc_macro2::Ident,
    pattern_ident: &proc_macro2::Ident,
) -> TokenStream2 {
    let WrapperConfig {
        ident,
        fixtures,
        step_args,
        datatable,
        docstring,
        pattern,
        ..
    } = *config;
    let declares = gen_fixture_decls(fixtures, ident);
    let captured: Vec<_> = step_args
        .iter()
        .enumerate()
        .map(|(idx, _)| {
            let index = syn::Index::from(idx + 1); // +1 to skip the full match at index 0
            quote! { captures.get(#index).map(|m| m.as_str()).unwrap_or_default() }
        })
        .collect();
    let step_arg_parses = gen_step_parses(step_args, &captured);
    let datatable_decl = gen_datatable_decl(datatable, pattern);
    let docstring_decl = gen_docstring_decl(docstring, pattern);
    let arg_idents = fixtures
        .iter()
        .map(|f| &f.pat)
        .chain(step_args.iter().map(|a| &a.pat))
        .chain(datatable.iter().map(|d| &d.pat))
        .chain(docstring.iter().map(|d| &d.pat));
    quote! {
        static #pattern_ident: rstest_bdd::StepPattern = rstest_bdd::StepPattern::new(#pattern);

        fn #wrapper_ident(
            ctx: &rstest_bdd::StepContext<'_>,
            text: &str,
            _docstring: Option<&str>,
            _table: Option<&[&[&str]]>,
        ) -> Result<(), String> {
            use std::panic::{catch_unwind, AssertUnwindSafe};

            let captures = #pattern_ident
                .regex()
                .captures(text)
                .ok_or_else(|| format!(
                    "Step text '{}' does not match pattern '{}'",
                    text,
                    #pattern
                ))?;

            #(#declares)*
            #(#step_arg_parses)*
            #datatable_decl
            #docstring_decl

            catch_unwind(AssertUnwindSafe(|| {
                #ident(#(#arg_idents),*);
                Ok(())
            }))
            .map_err(|e| format!(
                "Panic in step '{}', function '{}': {:?}",
                #pattern,
                stringify!(#ident),
                e
            ))?
        }
    }
}

/// Generate fixture registration and inventory code for the wrapper.
///
/// # Examples
/// ```rust,ignore
/// # use syn::parse_quote;
/// # use proc_macro2::Ident;
/// # let ident = Ident::new("step", proc_macro2::Span::call_site());
/// # let config = WrapperConfig { ident: &ident, fixtures: &[], step_args: &[], datatable: None, pattern: &parse_quote!(""), keyword: rstest_bdd::StepKeyword::Given };
/// # let (wrapper_ident, const_ident, pattern_ident) = generate_wrapper_identifiers(config.ident, 0);
/// let tokens = generate_registration_code(&config, &pattern_ident, &wrapper_ident, &const_ident);
/// assert!(tokens.to_string().contains("rstest_bdd"));
/// ```
fn generate_registration_code(
    config: &WrapperConfig<'_>,
    pattern_ident: &proc_macro2::Ident,
    wrapper_ident: &proc_macro2::Ident,
    const_ident: &proc_macro2::Ident,
) -> TokenStream2 {
    let WrapperConfig {
        fixtures, keyword, ..
    } = config;
    let fixture_names: Vec<_> = fixtures
        .iter()
        .map(|FixtureArg { name, .. }| {
            let s = name.to_string();
            quote! { #s }
        })
        .collect();
    let fixture_len = fixture_names.len();
    let keyword_token = keyword_to_token(*keyword);
    quote! {
        const #const_ident: [&'static str; #fixture_len] = [#(#fixture_names),*];
        const _: [(); #fixture_len] = [(); #const_ident.len()];

        rstest_bdd::step!(@pattern #keyword_token, &#pattern_ident, #wrapper_ident, &#const_ident);
    }
}

/// Generate the wrapper function and inventory registration.
pub(crate) fn generate_wrapper_code(config: &WrapperConfig<'_>) -> TokenStream2 {
    let id = COUNTER.fetch_add(1, Ordering::SeqCst);
    let (wrapper_ident, const_ident, pattern_ident) =
        generate_wrapper_identifiers(config.ident, id);
    let body = generate_wrapper_body(config, &wrapper_ident, &pattern_ident);
    let registration =
        generate_registration_code(config, &pattern_ident, &wrapper_ident, &const_ident);

    quote! {
        #body
        #registration
    }
}
