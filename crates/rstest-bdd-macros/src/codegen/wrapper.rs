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

/// Extract fixture and step arguments from a function signature.
pub(crate) fn extract_args(
    func: &mut syn::ItemFn,
) -> syn::Result<(Vec<FixtureArg>, Vec<StepArg>, Option<DataTableArg>)> {
    let mut fixtures = Vec::new();
    let mut step_args = Vec::new();
    let mut datatable = None;

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
        } else if datatable.is_none() && pat == "datatable" && is_vec_vec_string(&ty) {
            datatable = Some(DataTableArg { pat });
        } else {
            step_args.push(StepArg { pat, ty });
        }
    }

    Ok((fixtures, step_args, datatable))
}

fn is_vec_vec_string(ty: &syn::Type) -> bool {
    use syn::{GenericArgument, PathArguments, Type};

    let Type::Path(tp) = ty else { return false };
    let Some(seg) = tp.path.segments.first() else {
        return false;
    };
    if seg.ident != "Vec" {
        return false;
    }
    let PathArguments::AngleBracketed(inner) = &seg.arguments else {
        return false;
    };
    let Some(GenericArgument::Type(Type::Path(inner_tp))) = inner.args.first() else {
        return false;
    };
    let Some(inner_seg) = inner_tp.path.segments.first() else {
        return false;
    };
    if inner_seg.ident != "Vec" {
        return false;
    }
    let PathArguments::AngleBracketed(inner2) = &inner_seg.arguments else {
        return false;
    };
    let Some(GenericArgument::Type(Type::Path(str_tp))) = inner2.args.first() else {
        return false;
    };
    str_tp.path.is_ident("String")
}

/// Configuration required to generate a wrapper.
pub(crate) struct WrapperConfig<'a> {
    pub(crate) ident: &'a syn::Ident,
    pub(crate) fixtures: &'a [FixtureArg],
    pub(crate) step_args: &'a [StepArg],
    pub(crate) datatable: Option<&'a DataTableArg>,
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

/// Generate the wrapper function and inventory registration.
pub(crate) fn generate_wrapper_code(config: &WrapperConfig<'_>) -> TokenStream2 {
    let WrapperConfig {
        ident,
        fixtures,
        step_args,
        datatable,
        pattern,
        keyword,
    } = config;
    let id = COUNTER.fetch_add(1, Ordering::SeqCst);
    let wrapper_ident = format_ident!("__rstest_bdd_wrapper_{}_{}", ident, id);
    let ident_upper = ident.to_string().to_uppercase();
    let const_ident = format_ident!("__RSTEST_BDD_FIXTURES_{}_{}", ident_upper, id);
    let pattern_ident = format_ident!("__RSTEST_BDD_PATTERN_{}_{}", ident_upper, id);

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
    let datatable_decl = datatable.map(|DataTableArg { pat }| {
        quote! {
            let #pat: Vec<Vec<String>> = _table
                .ok_or_else(|| format!("Step '{}' requires a data table", #pattern))?
                .iter()
                .map(|row| row.iter().map(|cell| cell.to_string()).collect())
                .collect();
        }
    });
    let arg_idents = fixtures
        .iter()
        .map(|f| &f.pat)
        .chain(step_args.iter().map(|a| &a.pat))
        .chain(datatable.iter().map(|d| &d.pat));

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
        static #pattern_ident: rstest_bdd::StepPattern = rstest_bdd::StepPattern::new(#pattern);

        fn #wrapper_ident(
            ctx: &rstest_bdd::StepContext<'_>,
            text: &str,
            _table: Option<&[&[&str]]>,
        ) -> Result<(), String> {
            use std::panic::{catch_unwind, AssertUnwindSafe};

            let captures = #pattern_ident
                .regex()
                .captures(text)
                .ok_or_else(|| format!("Step text '{}' does not match pattern '{}'", text, #pattern))?;

            #(#declares)*
            #(#step_arg_parses)*
            #datatable_decl

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

        const #const_ident: [&'static str; #fixture_len] = [#(#fixture_names),*];
        const _: [(); #fixture_len] = [(); #const_ident.len()];

        rstest_bdd::step!(@pattern #keyword_token, &#pattern_ident, #wrapper_ident, &#const_ident);
    }
}
