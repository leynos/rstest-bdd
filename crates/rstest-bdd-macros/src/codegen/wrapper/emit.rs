//! Code emission helpers for wrapper generation.

use super::args::{ArgumentCollections, CallArg, DataTableArg, DocStringArg, FixtureArg, StepArg};
use super::arguments::{
    PreparedArgs, StepMeta, collect_ordered_arguments, prepare_argument_processing,
    step_error_tokens,
};
use crate::utils::ident::sanitize_ident;
use proc_macro2::TokenStream as TokenStream2;
use quote::{format_ident, quote};
use std::sync::atomic::{AtomicUsize, Ordering};

/// Configuration required to generate a wrapper.
pub(crate) struct WrapperConfig<'a> {
    pub(crate) ident: &'a syn::Ident,
    pub(crate) fixtures: &'a [FixtureArg],
    pub(crate) step_args: &'a [StepArg],
    pub(crate) datatable: Option<&'a DataTableArg>,
    pub(crate) docstring: Option<&'a DocStringArg>,
    pub(crate) pattern: &'a syn::LitStr,
    pub(crate) keyword: crate::StepKeyword,
    pub(crate) call_order: &'a [CallArg],
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
    let path = crate::codegen::rstest_bdd_path();
    quote! {
        static #pattern_ident: #path::StepPattern =
            #path::StepPattern::new(#pattern);
    }
}

/// Prepared wrapper inputs consumed by `assemble_wrapper_function`.
struct WrapperAssembly<'a> {
    meta: StepMeta<'a>,
    prepared: PreparedArgs,
    arg_idents: &'a [&'a syn::Ident],
    capture_count: usize,
}

struct WrapperErrors {
    placeholder: TokenStream2,
    panic: TokenStream2,
    execution: TokenStream2,
    capture_mismatch: TokenStream2,
}

fn prepare_wrapper_errors(meta: StepMeta<'_>, text_ident: &proc_macro2::Ident) -> WrapperErrors {
    let StepMeta { pattern, ident } = meta;
    let execution_error = format_ident!("ExecutionError");
    let panic_error = format_ident!("PanicError");
    let placeholder = step_error_tokens(
        &execution_error,
        pattern,
        ident,
        &quote! {
            format!(
                "Step text '{}' does not match pattern '{}': {}",
                #text_ident,
                #pattern,
                e
            )
        },
    );
    let panic = step_error_tokens(&panic_error, pattern, ident, &quote! { message });
    let execution = step_error_tokens(&execution_error, pattern, ident, &quote! { message });
    let capture_mismatch = step_error_tokens(
        &execution_error,
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

    WrapperErrors {
        placeholder,
        panic,
        execution,
        capture_mismatch,
    }
}

/// Assemble the final wrapper function using prepared components.
fn assemble_wrapper_function(
    wrapper_ident: &proc_macro2::Ident,
    pattern_ident: &proc_macro2::Ident,
    ctx_ident: &proc_macro2::Ident,
    text_ident: &proc_macro2::Ident,
    assembly: WrapperAssembly<'_>,
) -> TokenStream2 {
    let WrapperAssembly {
        meta,
        prepared,
        arg_idents,
        capture_count,
    } = assembly;
    let PreparedArgs {
        declares,
        step_arg_parses,
        datatable_decl,
        docstring_decl,
    } = prepared;
    let WrapperErrors {
        placeholder: placeholder_err,
        panic: panic_err,
        execution: exec_err,
        capture_mismatch: capture_mismatch_err,
    } = prepare_wrapper_errors(meta, text_ident);
    let StepMeta { pattern: _, ident } = meta;
    let expected = capture_count;
    let path = crate::codegen::rstest_bdd_path();
    let call = quote! { #ident(#(#arg_idents),*) };
    let call_expr = quote! { #path::IntoStepResult::into_step_result(#call) };

    quote! {
        fn #wrapper_ident(
            #ctx_ident: &#path::StepContext<'_>,
            #text_ident: &str,
            _docstring: Option<&str>,
            _table: Option<&[&[&str]]>,
        ) -> Result<Option<Box<dyn std::any::Any>>, #path::StepError> {
            use std::panic::{catch_unwind, AssertUnwindSafe};

            let captures = #path::extract_placeholders(&#pattern_ident, #text_ident.into())
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
                #call_expr
            }))
                .map_err(|e| {
                    let message = #path::panic_message(e.as_ref());
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

    let ctx_ident = format_ident!("__rstest_bdd_ctx");
    let text_ident = format_ident!("__rstest_bdd_text");
    let collections = ArgumentCollections {
        fixtures,
        step_args,
        datatable,
        docstring,
    };
    let step_meta = StepMeta { pattern, ident };
    let signature = generate_wrapper_signature(pattern, pattern_ident);
    let prepared = prepare_argument_processing(&collections, step_meta, &ctx_ident);
    let arg_idents = collect_ordered_arguments(call_order, &collections);
    let wrapper_fn = assemble_wrapper_function(
        wrapper_ident,
        pattern_ident,
        &ctx_ident,
        &text_ident,
        WrapperAssembly {
            meta: step_meta,
            prepared,
            arg_idents: &arg_idents,
            capture_count: step_args.len(),
        },
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
    let fixture_names: Vec<_> = config
        .fixtures
        .iter()
        .map(|FixtureArg { name, .. }| {
            let s = name.to_string();
            quote! { #s }
        })
        .collect();
    let fixture_len = fixture_names.len();
    let keyword = config.keyword;
    let path = crate::codegen::rstest_bdd_path();
    quote! {
        const #const_ident: [&'static str; #fixture_len] = [#(#fixture_names),*];
        const _: [(); #fixture_len] = [(); #const_ident.len()];

        #path::step!(@pattern #keyword, &#pattern_ident, #wrapper_ident, &#const_ident);
    }
}

/// Generate the wrapper function and inventory registration.
pub(crate) fn generate_wrapper_code(config: &WrapperConfig<'_>) -> TokenStream2 {
    // Relaxed ordering suffices: the counter only ensures a unique suffix and
    // is not used for synchronisation with other data.
    let id = COUNTER.fetch_add(1, Ordering::Relaxed);
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
        #[expect(clippy::expect_used, reason = "raw identifiers are test inputs")]
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
