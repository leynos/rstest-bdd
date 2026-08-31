//! Codegen equivalence between `#[harness_context]` and the legacy spelling.
//!
//! The roadmap contract says both spellings must generate byte-identical
//! wrapper code. These tests render the wrapper for each spelling and compare
//! the token streams as strings, and also record an `insta` snapshot of the
//! marker spelling so any future drift in the emitted shape shows up in
//! review.
//!
//! Rendering consumes the process-global wrapper counter
//! (`reset_wrapper_counter_for_tests`), so every test in this module must be
//! `#[serial]`; otherwise the generated identifiers race between threads.

use std::collections::HashSet;

use proc_macro2::TokenStream;
use quote::ToTokens;
use serial_test::serial;
use syn::parse_quote;

use super::{
    WrapperConfig,
    args::{ExtractedArgs, extract_args},
    emit::reset_wrapper_counter_for_tests,
    generate_wrapper_code,
};
use crate::{StepKeyword, return_classifier::StepReturnStrategy};

/// Build a `WrapperConfig` for a single-parameter step function source.
///
/// The step must carry exactly one argument (the harness context, in whichever
/// spelling the caller passes) and the step is modelled as a `When` with no
/// placeholders, keeping the generated wrapper focused on the fixture borrow.
fn wrapper_for_step(src: &str) -> TokenStream {
    let mut func: syn::ItemFn = match syn::parse_str(src) {
        Ok(func) => func,
        Err(error) => panic!("fixture step must parse: {error}"),
    };
    let mut placeholders = HashSet::new();
    let args: ExtractedArgs = match extract_args(&mut func, &mut placeholders) {
        Ok(args) => args,
        Err(error) => panic!("fixture step must classify: {error}"),
    };
    let config = WrapperConfig {
        ident: &func.sig.ident,
        is_async_step: false,
        args: &args,
        pattern: &parse_quote!("a context step"),
        keyword: StepKeyword::When,
        placeholder_names: &[],
        placeholder_hints: &[],
        capture_count: 0,
        strategy: StepReturnStrategy::Unit,
    };
    generate_wrapper_code(&config)
}

/// Render a step source to a string, resetting the global wrapper counter so
/// the generated identifiers are deterministic.
fn render(src: &str) -> String {
    reset_wrapper_counter_for_tests();
    wrapper_for_step(src).to_token_stream().to_string()
}

#[test]
#[serial]
fn marker_and_from_spelling_generate_identical_wrapper_code() {
    use pretty_assertions::assert_eq;

    let via_marker = render("fn record(#[harness_context] ctx: &TestCtx) {}");
    let via_from = render("fn record(#[from(rstest_bdd_harness_context)] ctx: &TestCtx) {}");

    assert_eq!(via_marker, via_from);
}

#[test]
#[serial]
fn marker_and_parameter_named_spelling_generate_identical_wrapper_code() {
    use pretty_assertions::assert_eq;

    let via_marker = render("fn record(#[harness_context] ctx: &TestCtx) {}");
    let by_name = render("fn record(rstest_bdd_harness_context: &TestCtx) {}");

    assert_eq!(via_marker, by_name);
}

#[test]
#[serial]
fn marker_spelling_generated_wrapper_snapshot() {
    let rendered = render("fn record(#[harness_context] ctx: &TestCtx) {}");
    insta::assert_snapshot!(rendered);
}
