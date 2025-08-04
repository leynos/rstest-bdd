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

/// Extract fixture and step arguments from a function signature.
pub(crate) fn extract_args(func: &mut syn::ItemFn) -> syn::Result<(Vec<FixtureArg>, Vec<StepArg>)> {
    let mut fixtures = Vec::new();
    let mut step_args = Vec::new();

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
        } else {
            step_args.push(StepArg { pat, ty });
        }
    }

    Ok((fixtures, step_args))
}

/// Configuration required to generate a wrapper.
pub(crate) struct WrapperConfig<'a> {
    pub(crate) ident: &'a syn::Ident,
    pub(crate) fixtures: &'a [FixtureArg],
    pub(crate) step_args: &'a [StepArg],
    pub(crate) pattern: &'a syn::LitStr,
    pub(crate) keyword: rstest_bdd::StepKeyword,
}

/// Generate declarations for fixture values.
fn gen_fixture_decls(fixtures: &[FixtureArg]) -> Vec<TokenStream2> {
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
                quote! { .clone() }
            };
            quote! {
                let #pat: #ty = ctx
                    .get::<#lookup_ty>(stringify!(#name))
                    .unwrap_or_else(|| panic!(
                        "missing fixture '{}' of type '{}'",
                        stringify!(#name),
                        stringify!(#lookup_ty),
                    ))
                    #clone_suffix;
            }
        })
        .collect()
}

/// Generate code to parse step arguments from regex captures.
fn gen_step_parses(step_args: &[StepArg]) -> Vec<TokenStream2> {
    step_args
        .iter()
        .enumerate()
        .map(|(idx, StepArg { pat, ty })| {
            let index = syn::Index::from(idx);
            quote! {
                let #pat: #ty = captures[#index]
                    .parse()
                    .unwrap_or_else(|_| panic!(
                        "failed to parse argument {} as {}",
                        #index,
                        stringify!(#ty)
                    ));
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
        pattern,
        keyword,
    } = config;
    let id = COUNTER.fetch_add(1, Ordering::SeqCst);
    let wrapper_ident = format_ident!("__rstest_bdd_wrapper_{}_{}", ident, id);
    let ident_upper = ident.to_string().to_uppercase();
    let const_ident = format_ident!("__RSTEST_BDD_FIXTURES_{}_{}", ident_upper, id);
    let pattern_ident = format_ident!("__RSTEST_BDD_PATTERN_{}_{}", ident_upper, id);

    let declares = gen_fixture_decls(fixtures);
    let step_arg_parses = gen_step_parses(step_args);
    let arg_idents = fixtures
        .iter()
        .map(|f| &f.pat)
        .chain(step_args.iter().map(|a| &a.pat));

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

        fn #wrapper_ident(ctx: &rstest_bdd::StepContext<'_>, text: &str) {
            use std::panic::{catch_unwind, AssertUnwindSafe};

            let result = catch_unwind(AssertUnwindSafe(|| {
                #(#declares)*
                let captures = rstest_bdd::extract_placeholders(&#pattern_ident, text.into())
                    .expect("pattern mismatch");
                #(#step_arg_parses)*
                #ident(#(#arg_idents),*);
            }));

            if let Err(e) = result {
                panic!(
                    "Panic in step '{}', function '{}': {:?}",
                    #pattern,
                    stringify!(#ident),
                    e
                );
            }
        }

        const #const_ident: [&'static str; #fixture_len] = [#(#fixture_names),*];
        const _: [(); #fixture_len] = [(); #const_ident.len()];

        rstest_bdd::step!(#keyword_token, #pattern, #wrapper_ident, &#const_ident);
    }
}
