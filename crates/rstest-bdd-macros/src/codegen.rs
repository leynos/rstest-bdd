//! Code generation utilities for step registration.

use proc_macro2::TokenStream as TokenStream2;
use quote::{format_ident, quote};
use std::sync::atomic::{AtomicUsize, Ordering};

use crate::args::{FixtureArg, StepArg, gen_arg_decls_and_idents, prepare_arguments};

pub(crate) struct WrapperConfig<'a> {
    pub(crate) ident: &'a syn::Ident,
    pub(crate) fixtures: &'a [FixtureArg],
    pub(crate) step_args: &'a [StepArg],
    pub(crate) pattern: &'a syn::LitStr,
    pub(crate) keyword: rstest_bdd::StepKeyword,
}

pub(crate) fn quote_keyword(kw: rstest_bdd::StepKeyword) -> TokenStream2 {
    match kw {
        rstest_bdd::StepKeyword::Given => quote! { rstest_bdd::StepKeyword::Given },
        rstest_bdd::StepKeyword::When => quote! { rstest_bdd::StepKeyword::When },
        rstest_bdd::StepKeyword::Then => quote! { rstest_bdd::StepKeyword::Then },
        rstest_bdd::StepKeyword::And => quote! { rstest_bdd::StepKeyword::And },
        rstest_bdd::StepKeyword::But => quote! { rstest_bdd::StepKeyword::But },
    }
}

fn generate_identifiers(ident: &syn::Ident, id: usize) -> (syn::Ident, syn::Ident, syn::Ident) {
    let wrapper_ident = format_ident!("__rstest_bdd_wrapper_{}_{}", ident, id);
    let const_ident = format_ident!("__rstest_bdd_fixtures_{}_{}", ident, id);
    let pattern_ident = format_ident!("__rstest_bdd_pattern_{}_{}", ident, id);
    (wrapper_ident, const_ident, pattern_ident)
}

fn fixture_metadata(fixtures: &[FixtureArg]) -> (Vec<TokenStream2>, usize) {
    let names: Vec<_> = fixtures
        .iter()
        .map(|FixtureArg { name, .. }| {
            let s = name.to_string();
            quote! { #s }
        })
        .collect();
    let len = names.len();
    (names, len)
}

fn generate_captures_stmt(step_args: &[StepArg], pattern_ident: &syn::Ident) -> TokenStream2 {
    if step_args.is_empty() {
        quote! {
            let _ = #pattern_ident
                .captures(text.into())
                .expect("pattern mismatch");
        }
    } else {
        quote! {
            let captures = #pattern_ident
                .captures(text.into())
                .expect("pattern mismatch");
        }
    }
}

pub(crate) fn generate_wrapper_code(config: &WrapperConfig<'_>) -> TokenStream2 {
    let WrapperConfig {
        ident,
        fixtures,
        step_args,
        pattern,
        keyword,
    } = config;
    let id = COUNTER.fetch_add(1, Ordering::SeqCst);
    let (wrapper_ident, const_ident, pattern_ident) = generate_identifiers(ident, id);

    let args = prepare_arguments(fixtures, step_args);
    let (declares, arg_idents) = gen_arg_decls_and_idents(&args);

    let (fixture_names, fixture_len) = fixture_metadata(fixtures);
    let keyword_token = quote_keyword(*keyword);
    let captures_stmt = generate_captures_stmt(step_args, &pattern_ident);

    quote! {
        #[allow(non_upper_case_globals)]
        static #pattern_ident: rstest_bdd::StepPattern = rstest_bdd::StepPattern::new(#pattern);

        fn #wrapper_ident(ctx: &rstest_bdd::StepContext<'_>, text: &str) {
            #captures_stmt
            #(#declares)*
            #ident(#(#arg_idents),*);
        }

        #[allow(non_upper_case_globals)]
        const #const_ident: [&'static str; #fixture_len] = [#(#fixture_names),*];
        const _: [(); #fixture_len] = [(); #const_ident.len()];

        rstest_bdd::submit! {
            rstest_bdd::Step {
                keyword: #keyword_token,
                pattern: &#pattern_ident,
                run: #wrapper_ident,
                fixtures: &#const_ident,
                file: file!(),
                line: line!(),
            }
        }
    }
}

static COUNTER: AtomicUsize = AtomicUsize::new(0);
