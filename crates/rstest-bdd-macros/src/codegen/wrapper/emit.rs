//! Code emission helpers for wrapper generation.

use super::args::{ArgumentCollections, CallArg, DataTableArg, DocStringArg, FixtureArg, StepArg};
use crate::utils::ident::sanitize_ident;
use proc_macro2::TokenStream as TokenStream2;
use quote::{format_ident, quote};
use std::sync::atomic::{AtomicUsize, Ordering};

#[derive(Copy, Clone)]
struct StepMeta<'a> {
    pattern: &'a syn::LitStr,
    ident: &'a syn::Ident,
}

/// Quote construction for [`StepError`] variants sharing `pattern`,
/// `function` and `message` fields.
fn step_error_tokens(
    variant: &syn::Ident,
    pattern: &syn::LitStr,
    ident: &syn::Ident,
    message: &TokenStream2,
) -> TokenStream2 {
    quote! {
        rstest_bdd::StepError::#variant {
            pattern: #pattern.to_string(),
            function: stringify!(#ident).to_string(),
            message: #message,
        }
    }
}

/// Generate declaration for an optional argument, mapping absence to
/// `StepError::ExecutionError`.
fn gen_optional_decl<T, F>(
    arg: Option<&T>,
    meta: StepMeta<'_>,
    error_msg: &str,
    generator: F,
) -> Option<TokenStream2>
where
    F: FnOnce(&T) -> (syn::Ident, TokenStream2, TokenStream2),
{
    arg.map(|arg_value| {
        let (pat, ty, expr) = generator(arg_value);
        let StepMeta { pattern, ident } = meta;
        let missing_err = step_error_tokens(
            &format_ident!("ExecutionError"),
            pattern,
            ident,
            &quote! { format!("Step '{}' {}", #pattern, #error_msg) },
        );
        let convert_err = step_error_tokens(
            &format_ident!("ExecutionError"),
            pattern,
            ident,
            &quote! { format!("failed to convert auxiliary argument for step '{}'", #pattern) },
        );
        quote! {
            let #pat: #ty = #expr
                .ok_or_else(|| #missing_err)?
                .try_into()
                .map_err(|_e| #convert_err)?;
        }
    })
}

/// Generate declaration for a data table argument.
fn gen_datatable_decl(
    datatable: Option<&DataTableArg>,
    pattern: &syn::LitStr,
    ident: &syn::Ident,
) -> Option<TokenStream2> {
    gen_optional_decl(
        datatable,
        StepMeta { pattern, ident },
        "requires a data table",
        |DataTableArg { pat, ty }| {
            let pat = pat.clone();
            let declared_ty = ty.clone();
            let ty = quote! { #declared_ty };
            let expr = quote! {
                _table.map(|t| {
                    t.iter()
                        .map(|row| row.iter().map(|cell| cell.to_string()).collect::<Vec<String>>())
                        .collect::<Vec<Vec<String>>>()
                })
            };
            (pat, ty, expr)
        },
    )
}

/// Generate declaration for a doc string argument.
///
/// Step functions require an owned `String`, so the wrapper copies the block.
fn gen_docstring_decl(
    docstring: Option<&DocStringArg>,
    pattern: &syn::LitStr,
    ident: &syn::Ident,
) -> Option<TokenStream2> {
    gen_optional_decl(
        docstring,
        StepMeta { pattern, ident },
        "requires a doc string",
        |DocStringArg { pat }| {
            let pat = pat.clone();
            let ty = quote! { String };
            let expr = quote! { _docstring.map(|s| s.to_owned()) };
            (pat, ty, expr)
        },
    )
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
                    .ok_or_else(|| rstest_bdd::StepError::MissingFixture {
                        name: stringify!(#name).to_string(),
                        ty: stringify!(#lookup_ty).to_string(),
                        step: stringify!(#ident).to_string(),
                    })?;
            }
        })
        .collect()
}

/// Generate code to parse step arguments from regex captures.
fn gen_step_parses(
    step_args: &[StepArg],
    captured: &[TokenStream2],
    meta: StepMeta<'_>,
) -> Vec<TokenStream2> {
    let StepMeta { pattern, ident } = meta;
    step_args
        .iter()
        .zip(captured.iter().enumerate())
        .map(|(StepArg { pat, ty }, (idx, capture))| {
            let raw_ident = format_ident!("__raw{}", idx);
            let missing_cap_err = step_error_tokens(
                &format_ident!("ExecutionError"),
                pattern,
                ident,
                &quote! {
                    format!(
                        "pattern '{}' missing capture for argument '{}'",
                        #pattern,
                        stringify!(#pat),
                    )
                },
            );
            let parse_err = step_error_tokens(
                &format_ident!("ExecutionError"),
                pattern,
                ident,
                &quote! {
                    format!(
                        "failed to parse argument '{}' of type '{}' from pattern '{}' with captured value: '{:?}'",
                        stringify!(#pat),
                        stringify!(#ty),
                        #pattern,
                        #raw_ident,
                    )
                },
            );
            quote! {
                let #raw_ident = #capture.ok_or_else(|| #missing_cap_err)?;
                let #pat: #ty = (#raw_ident).parse().map_err(|_| #parse_err)?;
            }
        })
        .collect()
}

static COUNTER: AtomicUsize = AtomicUsize::new(0);

/// Generate unique identifiers for the wrapper components.
///
/// The provided step function identifier may contain Unicode. It is
/// sanitized to ASCII before constructing constant names to avoid emitting
/// invalid identifiers.
///
/// Returns identifiers for the wrapper function, fixture array constant, and
/// pattern constant.
fn generate_wrapper_identifiers(
    ident: &syn::Ident,
    id: usize,
) -> (proc_macro2::Ident, proc_macro2::Ident, proc_macro2::Ident) {
    let ident_sanitized = sanitize_ident(&ident.to_string());
    let wrapper_ident = format_ident!("__rstest_bdd_wrapper_{}_{}", ident_sanitized, id);
    let ident_upper = ident_sanitized.to_ascii_uppercase();
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
            let index = syn::Index::from(idx);
            quote! { captures.get(#index).map(|m| m.as_str()) }
        })
        .collect();
    let step_arg_parses = gen_step_parses(
        config.step_args,
        &captured,
        StepMeta {
            pattern: config.pattern,
            ident: config.ident,
        },
    );
    let datatable_decl = gen_datatable_decl(config.datatable, config.pattern, config.ident);
    let docstring_decl = gen_docstring_decl(config.docstring, config.pattern, config.ident);
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
    capture_count: usize,
) -> TokenStream2 {
    let (declares, step_arg_parses, datatable_decl, docstring_decl) = arg_processing;
    let placeholder_err = step_error_tokens(
        &format_ident!("ExecutionError"),
        pattern,
        ident,
        &quote! {
            format!(
                "Step text '{}' does not match pattern '{}': {}",
                text,
                #pattern,
                e
            )
        },
    );
    let panic_err = step_error_tokens(
        &format_ident!("PanicError"),
        pattern,
        ident,
        &quote! { message },
    );
    let exec_err = step_error_tokens(
        &format_ident!("ExecutionError"),
        pattern,
        ident,
        &quote! { message },
    );
    let expected = capture_count;
    let capture_mismatch_err = step_error_tokens(
        &format_ident!("ExecutionError"),
        pattern,
        ident,
        &quote! {
            format!(
                "pattern '{}' produced {} captures but step '{}' expects {}",
                #pattern,
                captures.len(),
                stringify!(#ident),
                expected
            )
        },
    );
    quote! {
        fn #wrapper_ident(
            ctx: &rstest_bdd::StepContext<'_>,
            text: &str,
            _docstring: Option<&str>,
            _table: Option<&[&[&str]]>,
        ) -> Result<(), rstest_bdd::StepError> {
            use std::panic::{catch_unwind, AssertUnwindSafe};

            let captures = rstest_bdd::extract_placeholders(&#pattern_ident, text.into())
                .map_err(|e| #placeholder_err)?;
            let expected: usize = #expected;
            if captures.len() != expected {
                return Err(#capture_mismatch_err);
            }

            #(#declares)*
            #(#step_arg_parses)*
            #datatable_decl
            #docstring_decl

            catch_unwind(AssertUnwindSafe(|| {
                rstest_bdd::IntoStepResult::into_step_result(#ident(#(#arg_idents),*))
            }))
                .map_err(|e| {
                    let message = rstest_bdd::panic_message(e.as_ref());
                    #panic_err
                })
                .and_then(|res| res.map_err(|message| #exec_err))
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
        step_args.len(),
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
    quote! {
        const #const_ident: [&'static str; #fixture_len] = [#(#fixture_names),*];
        const _: [(); #fixture_len] = [(); #const_ident.len()];

        rstest_bdd::step!(@pattern #keyword, &#pattern_ident, #wrapper_ident, &#const_ident);
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

#[cfg(test)]
mod tests {
    //! Tests for wrapper code generation helpers.

    use super::generate_wrapper_identifiers;
    use crate::utils::ident::sanitize_ident;
    use rstest::rstest;
    use syn::parse_str;

    #[rstest]
    #[case(
        "préférence",
        3,
        "__rstest_bdd_wrapper_pr_f_rence_3",
        "__RSTEST_BDD_FIXTURES_PR_F_RENCE_3",
        "__RSTEST_BDD_PATTERN_PR_F_RENCE_3"
    )]
    #[case(
        "数字",
        2,
        "__rstest_bdd_wrapper___2",
        "__RSTEST_BDD_FIXTURES___2",
        "__RSTEST_BDD_PATTERN___2"
    )]
    #[case(
        "_1er_pas",
        4,
        "__rstest_bdd_wrapper__1er_pas_4",
        "__RSTEST_BDD_FIXTURES__1ER_PAS_4",
        "__RSTEST_BDD_PATTERN__1ER_PAS_4"
    )]
    fn generates_ascii_only_idents(
        #[case] raw: &str,
        #[case] id: usize,
        #[case] expected_wrapper: &str,
        #[case] expected_const: &str,
        #[case] expected_pattern: &str,
    ) {
        #[expect(
            clippy::expect_used,
            reason = "tests ensure identifier parsing succeeds"
        )]
        let ident = parse_str::<syn::Ident>(raw).expect("parse identifier");
        let (wrapper_ident, const_ident, pattern_ident) = generate_wrapper_identifiers(&ident, id);

        // Verify wrapper ident derives from the sanitized base.
        let base = sanitize_ident(&ident.to_string());
        assert!(
            wrapper_ident.to_string().ends_with(&format!("{base}_{id}")),
            "wrapper ident must include sanitized base and id",
        );

        // Exact expectations
        assert_eq!(wrapper_ident.to_string(), expected_wrapper);
        assert_eq!(const_ident.to_string(), expected_const);
        assert_eq!(pattern_ident.to_string(), expected_pattern);

        // ASCII-only invariants
        assert!(wrapper_ident.to_string().is_ascii());
        assert!(const_ident.to_string().is_ascii());
        assert!(pattern_ident.to_string().is_ascii());
    }
}
