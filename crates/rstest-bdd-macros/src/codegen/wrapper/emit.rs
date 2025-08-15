//! Code emission helpers for wrapper generation.

use super::args::{ArgumentCollections, CallArg, DataTableArg, DocStringArg, FixtureArg, StepArg};
use crate::codegen::keyword_to_token;
use proc_macro2::TokenStream as TokenStream2;
use quote::{format_ident, quote};
use std::sync::atomic::{AtomicUsize, Ordering};

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
///
/// Step functions require an owned `String`, so the wrapper copies the block.
fn gen_docstring_decl(
    docstring: Option<&DocStringArg>,
    pattern: &syn::LitStr,
) -> Option<TokenStream2> {
    docstring.map(|DocStringArg { pat }| {
        quote! {
            let #pat: String = _docstring
                .ok_or_else(|| format!("Step '{}' requires a doc string", #pattern))?
                .to_owned();
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
    pub(crate) call_order: &'a [CallArg],
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
fn gen_step_parses(
    step_args: &[StepArg],
    captured: &[TokenStream2],
    pattern: &syn::LitStr,
) -> Vec<TokenStream2> {
    step_args
        .iter()
        .zip(captured.iter().enumerate())
        .map(|(StepArg { pat, ty }, (idx, capture))| {
            let raw_ident = format_ident!("__raw{}", idx);
            quote! {
                let #raw_ident = #capture.unwrap_or_else(|| {
                    panic!(
                        "pattern '{}' missing capture for argument '{}'",
                        #pattern,
                        stringify!(#pat),
                    )
                });
                let #pat: #ty = (#raw_ident).parse().unwrap_or_else(|_| {
                    panic!(
                        "failed to parse argument '{}' of type '{}' from pattern '{}' with captured value: '{:?}'",
                        stringify!(#pat),
                        stringify!(#ty),
                        #pattern,
                        #raw_ident,
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

/// Generate the `StepPattern` constant used by a wrapper.
fn generate_wrapper_signature(
    pattern: &syn::LitStr,
    pattern_ident: &proc_macro2::Ident,
) -> TokenStream2 {
    quote! {
        static #pattern_ident: rstest_bdd::StepPattern =
            rstest_bdd::StepPattern::new(#pattern);
    }
}

/// Generate declarations and parsing logic for wrapper arguments.
fn generate_argument_processing(
    config: &WrapperConfig<'_>,
) -> (
    Vec<TokenStream2>,
    Vec<TokenStream2>,
    Option<TokenStream2>,
    Option<TokenStream2>,
) {
    let declares = gen_fixture_decls(config.fixtures, config.ident);
    let captured: Vec<_> = config
        .step_args
        .iter()
        .enumerate()
        .map(|(idx, _)| {
            let index = syn::Index::from(idx + 1); // skip full match at index 0
            quote! { captures.get(#index).map(|m| m.as_str()) }
        })
        .collect();
    let step_arg_parses = gen_step_parses(config.step_args, &captured, config.pattern);
    let datatable_decl = gen_datatable_decl(config.datatable, config.pattern);
    let docstring_decl = gen_docstring_decl(config.docstring, config.pattern);
    (declares, step_arg_parses, datatable_decl, docstring_decl)
}

/// Collect argument identifiers in the order declared by the step function.
fn collect_ordered_arguments<'a>(
    call_order: &'a [CallArg],
    args: &ArgumentCollections<'a>,
) -> Vec<&'a syn::Ident> {
    call_order
        .iter()
        .map(|arg| match arg {
            CallArg::Fixture(i) =>
            {
                #[expect(
                    clippy::indexing_slicing,
                    reason = "indices validated during extraction"
                )]
                &args.fixtures[*i].pat
            }
            CallArg::StepArg(i) =>
            {
                #[expect(
                    clippy::indexing_slicing,
                    reason = "indices validated during extraction"
                )]
                &args.step_args[*i].pat
            }
            CallArg::DataTable =>
            {
                #[expect(clippy::expect_used, reason = "variant guarantees presence")]
                &args
                    .datatable
                    .expect("datatable present in call_order but not configured")
                    .pat
            }
            CallArg::DocString =>
            {
                #[expect(clippy::expect_used, reason = "variant guarantees presence")]
                &args
                    .docstring
                    .expect("docstring present in call_order but not configured")
                    .pat
            }
        })
        .collect()
}

/// Assemble the final wrapper function using prepared components.
fn assemble_wrapper_function(
    wrapper_ident: &proc_macro2::Ident,
    pattern_ident: &proc_macro2::Ident,
    arg_processing: (
        Vec<TokenStream2>,
        Vec<TokenStream2>,
        Option<TokenStream2>,
        Option<TokenStream2>,
    ),
    arg_idents: &[&syn::Ident],
    pattern: &syn::LitStr,
    ident: &syn::Ident,
) -> TokenStream2 {
    let (declares, step_arg_parses, datatable_decl, docstring_decl) = arg_processing;
    quote! {
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

/// Generate the wrapper function body and pattern constant.
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
        call_order,
        ..
    } = *config;

    let signature = generate_wrapper_signature(pattern, pattern_ident);
    let arg_processing = generate_argument_processing(config);
    let collections = ArgumentCollections {
        fixtures,
        step_args,
        datatable,
        docstring,
    };
    let arg_idents = collect_ordered_arguments(call_order, &collections);
    let wrapper_fn = assemble_wrapper_function(
        wrapper_ident,
        pattern_ident,
        arg_processing,
        &arg_idents,
        pattern,
        ident,
    );

    quote! {
        #signature
        #wrapper_fn
    }
}

/// Generate fixture registration and inventory code for the wrapper.
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
