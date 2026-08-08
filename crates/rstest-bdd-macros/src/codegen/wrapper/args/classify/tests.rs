//! Unit tests for the argument classifier helpers.

use std::collections::HashSet;

use proc_macro2::{Span, TokenStream as TokenStream2};
use quote::quote;
use rstest::{fixture, rstest};
use syn::{FnArg, parse_quote};

use super::*;

fn ident(name: &str) -> syn::Ident { syn::Ident::new(name, Span::call_site()) }

fn pat_type(tokens: TokenStream2) -> syn::PatType {
    match syn::parse2::<FnArg>(tokens) {
        Ok(FnArg::Typed(arg)) => arg,
        Ok(FnArg::Receiver(_)) => panic!("expected typed argument"),
        Err(err) => panic!("failed to parse argument: {err}"),
    }
}

fn pat_ident(arg: &syn::PatType) -> syn::Ident {
    match arg.pat.as_ref() {
        syn::Pat::Ident(pat) => pat.ident.clone(),
        other => panic!("expected identifier pattern, got {other:?}"),
    }
}

/// Helper to execute `classify_fixture_or_step` and return the results for assertion.
fn execute_classify_fixture_or_step(
    placeholders_init: HashSet<String>,
    arg_tokens: TokenStream2,
    pat_name: &str,
    ty_tokens: TokenStream2,
) -> (ExtractedArgs, bool, HashSet<String>) {
    let mut extracted = ExtractedArgs::default();
    let mut placeholders = placeholders_init;
    let mut arg: syn::PatType = match syn::parse2(arg_tokens) {
        Ok(parsed) => parsed,
        Err(err) => panic!("failed to parse argument: {err}"),
    };
    let pat = ident(pat_name);
    let ty: syn::Type = match syn::parse2(ty_tokens) {
        Ok(parsed) => parsed,
        Err(err) => panic!("failed to parse type: {err}"),
    };

    let mut ctx = ClassificationContext::new(&mut extracted, &mut placeholders);
    let handled = match classify_fixture_or_step(&mut ctx, &mut arg, pat, ty) {
        Ok(handled) => handled,
        Err(err) => panic!("classification should succeed: {err}"),
    };

    (extracted, handled, placeholders)
}

/// Helper to execute classification and assert the parameter matches a placeholder as a step.
fn assert_classifies_as_step(
    placeholders_init: HashSet<String>,
    arg_tokens: TokenStream2,
    pat_name: &str,
    ty_tokens: TokenStream2,
) {
    let (extracted, handled, placeholders) =
        execute_classify_fixture_or_step(placeholders_init, arg_tokens, pat_name, ty_tokens);
    assert!(handled);
    assert!(placeholders.is_empty());
    assert!(matches!(extracted.args.as_slice(), [Arg::Step { .. }]));
}

#[test]
fn context_new_links_borrows() {
    let mut extracted = ExtractedArgs::default();
    let mut placeholders = HashSet::from(["alpha".to_owned()]);
    {
        let ctx = ClassificationContext::new(&mut extracted, &mut placeholders);
        ctx.placeholders.clear();
        ctx.extracted.push(Arg::DocString {
            pat: ident("docstring"),
        });
    }
    assert!(placeholders.is_empty());
    assert!(matches!(
        extracted.args.first(),
        Some(Arg::DocString { .. })
    ));
}

#[test]
fn classify_fixture_or_step_claims_placeholder_as_step() {
    assert_classifies_as_step(
        HashSet::from(["value".to_owned()]),
        quote!(value: String),
        "value",
        quote!(String),
    );
}

#[test]
fn classify_fixture_or_step_falls_back_to_fixture() {
    let (extracted, handled, _) =
        execute_classify_fixture_or_step(HashSet::new(), quote!(dep: usize), "dep", quote!(usize));
    let pat = ident("dep");
    let name = ident("dep");

    assert!(handled);
    assert!(matches!(
        extracted.args.as_slice(),
        [Arg::Fixture { pat: fixture_pat, name: fixture_name, .. }]
        if fixture_pat == &pat && fixture_name == &name
    ));
}

/// Accumulator holding a `#[step_args]` struct that owns the `blocked`
/// placeholder, for the conflict-guard cases.
#[fixture]
fn step_struct_extracted() -> ExtractedArgs {
    let mut extracted = ExtractedArgs::default();
    let idx = extracted.push(Arg::StepStruct {
        pat: ident("args"),
        ty: parse_quote!(Args),
    });
    extracted.step_struct_idx = Some(idx);
    extracted.blocked_placeholders.insert("blocked".into());
    extracted
}

/// Classify `arg_tokens` against `extracted` and return the rejection message.
///
/// Panics when classification unexpectedly succeeds, so each caller asserts
/// only on the diagnostic it cares about.
fn classification_error(
    mut extracted: ExtractedArgs,
    arg_tokens: TokenStream2,
    pat_name: &str,
    ty_tokens: TokenStream2,
) -> String {
    let mut placeholders = HashSet::new();
    // Parse via `FnArg` so leading attributes are captured on the `PatType`;
    // a bare `PatType` parse would drop them.
    let mut arg = pat_type(arg_tokens);
    let pat = ident(pat_name);
    let ty: syn::Type = match syn::parse2(ty_tokens) {
        Ok(parsed) => parsed,
        Err(err) => panic!("failed to parse type: {err}"),
    };
    let mut ctx = ClassificationContext::new(&mut extracted, &mut placeholders);
    let Err(err) = classify_fixture_or_step(&mut ctx, &mut arg, pat, ty) else {
        panic!("classification should fail for {pat_name}");
    };
    err.to_string()
}

#[rstest]
// Both the last-wins (`#[from(a)] #[from(b)]`) and the silently-stripped
// bare-duplicate forms must fail.
#[case::duplicate_from_named(
    quote!(#[from(a)] #[from(b)] u: String),
    "u",
    quote!(String),
    "duplicate `#[from]` attribute"
)]
#[case::duplicate_from_bare(
    quote!(#[from] #[from] u: String),
    "u",
    quote!(String),
    "duplicate `#[from]` attribute"
)]
#[case::name_value_from(
    quote!(#[from = "db"] pool: String),
    "pool",
    quote!(String),
    "expects an identifier or no arguments"
)]
// `_1` normalizes to `1`, which is not a valid identifier; the classifier must
// reject it rather than emit a broken fixture name.
#[case::invalid_normalized_name(
    quote!(_1: usize),
    "_1",
    quote!(usize),
    "is not a valid identifier"
)]
fn classify_fixture_or_step_rejects_malformed_parameters(
    #[case] arg_tokens: TokenStream2,
    #[case] pat_name: &str,
    #[case] ty_tokens: TokenStream2,
    #[case] expected_fragment: &str,
) {
    let err = classification_error(ExtractedArgs::default(), arg_tokens, pat_name, ty_tokens);
    assert!(
        err.contains(expected_fragment),
        "expected {expected_fragment:?} in error, got {err:?}"
    );
}

#[rstest]
#[case::exact_name("blocked")]
// `_blocked` normalizes to `blocked`, which the `#[step_args]` struct owns.
// The guard must compare the normalized name, or the leading underscore would
// let the parameter shadow the struct field unnoticed.
#[case::underscore_normalized("_blocked")]
fn classify_fixture_or_step_respects_blocked_placeholders(
    step_struct_extracted: ExtractedArgs,
    #[case] pat_name: &str,
) {
    let arg_tokens: TokenStream2 = match format!("{pat_name}: String").parse() {
        Ok(tokens) => tokens,
        Err(err) => panic!("failed to parse argument tokens: {err}"),
    };
    let err = classification_error(step_struct_extracted, arg_tokens, pat_name, quote!(String));
    assert!(
        err.contains("#[step_args] cannot be combined with named step arguments"),
        "expected the step_args conflict diagnostic, got {err:?}"
    );
}

#[test]
fn classify_fixture_or_step_strips_from_attribute_on_success() {
    let mut extracted = ExtractedArgs::default();
    let mut placeholders = HashSet::new();
    let mut arg = pat_type(quote!(#[from(db)] pool: DbPool));
    let pat = ident("pool");
    let ty: syn::Type = parse_quote!(DbPool);
    let mut ctx = ClassificationContext::new(&mut extracted, &mut placeholders);
    if let Err(err) = classify_fixture_or_step(&mut ctx, &mut arg, pat, ty) {
        panic!("classification should succeed: {err}");
    }

    // The `#[from]` attribute must be stripped so the generated wrapper does
    // not re-emit it, and the explicit name overrides the parameter identifier.
    assert!(
        arg.attrs.is_empty(),
        "#[from] should be stripped on success"
    );
    let name = ident("db");
    assert!(matches!(
        extracted.args.as_slice(),
        [Arg::Fixture { name: fixture_name, .. }] if fixture_name == &name
    ));
}

#[test]
fn extract_step_struct_attribute_detects_marker() {
    let mut arg = pat_type(quote!(#[step_args] args: Args));
    match extract_step_struct_attribute(&mut arg) {
        Ok(true) => {}
        Ok(false) => panic!("attribute should be detected"),
        Err(err) => panic!("attribute parse failed: {err}"),
    }
    assert!(arg.attrs.is_empty());
}

#[test]
fn classify_step_struct_blocks_placeholders() {
    let mut extracted = ExtractedArgs::default();
    let mut placeholders = HashSet::from(["alpha".to_owned(), "beta".to_owned()]);
    let arg = pat_type(quote!(#[step_args] args: Args));

    match classify_step_struct(&mut extracted, &arg, &mut placeholders) {
        Ok(()) => {}
        Err(err) => panic!("step struct classification should succeed: {err}"),
    }

    assert!(placeholders.is_empty());
    assert_eq!(
        extracted.blocked_placeholders,
        HashSet::from(["alpha".to_owned(), "beta".to_owned()])
    );
    assert!(
        extracted
            .step_struct()
            .is_some_and(|step| step.pat == "args")
    );
}

#[test]
fn classify_fixture_or_step_matches_underscore_prefixed_param_to_placeholder() {
    assert_classifies_as_step(
        HashSet::from(["value".to_owned()]),
        quote!(_value: String),
        "_value",
        quote!(String),
    );
}

#[test]
fn classify_fixture_or_step_double_underscore_matches_single_underscore_placeholder() {
    assert_classifies_as_step(
        HashSet::from(["_value".to_owned()]),
        quote!(__value: String),
        "__value",
        quote!(String),
    );
}

#[test]
fn classify_fixture_or_step_double_underscore_does_not_match_plain_placeholder() {
    // Placeholder set only contains "value"; "__value" should NOT be classified as a step
    // and "value" should remain in the placeholder set (it is unmatched).
    let (extracted, handled, placeholders) = execute_classify_fixture_or_step(
        HashSet::from(["value".to_owned()]),
        quote!(__value: String),
        "__value",
        quote!(String),
    );
    assert!(handled);
    // Should be classified as a fixture since "__value" normalizes to "_value", not "value"
    assert!(matches!(extracted.args.as_slice(), [Arg::Fixture { .. }]));
    // "value" placeholder remains unconsumed
    assert!(placeholders.contains("value"));
}

#[rstest]
#[case("world: usize", "world", "world", "world")]
#[case("_world: usize", "_world", "_world", "world")]
#[case("__world: usize", "__world", "__world", "_world")]
fn classify_fixture_or_step_normalizes_implicit_fixture_name(
    #[case] arg_str: &str,
    #[case] param_name: &str,
    #[case] expected_pat: &str,
    #[case] expected_name: &str,
) {
    let arg_tokens: TokenStream2 = arg_str.parse().expect("case input should tokenize");
    let (extracted, handled, _) =
        execute_classify_fixture_or_step(HashSet::new(), arg_tokens, param_name, quote!(usize));
    let expected_pat_ident = ident(expected_pat);
    let expected_name_ident = ident(expected_name);

    assert!(handled);
    assert!(matches!(
        extracted.args.as_slice(),
        [Arg::Fixture { pat, name, .. }]
        if pat == &expected_pat_ident && name == &expected_name_ident
    ));
}

#[test]
fn classify_fixture_or_step_uses_parameter_span_for_normalized_fixture_name() {
    let mut extracted = ExtractedArgs::default();
    let mut placeholders = HashSet::new();
    let mut arg = pat_type(quote!(_world: usize));
    let pat = pat_ident(&arg);
    let pat_span = pat.span();
    let ty: syn::Type = parse_quote!(usize);
    let mut ctx = ClassificationContext::new(&mut extracted, &mut placeholders);
    let handled = match classify_fixture_or_step(&mut ctx, &mut arg, pat, ty) {
        Ok(handled) => handled,
        Err(err) => panic!("classification should succeed: {err}"),
    };

    assert!(handled);
    // Named predicate keeps the `matches!` guard within the two-branch limit.
    let spans_match = |name: &syn::Ident| {
        name.span().start() == pat_span.start() && name.span().end() == pat_span.end()
    };
    assert!(matches!(
        extracted.args.as_slice(),
        [Arg::Fixture { name, .. }]
        if name == &ident("world") && spans_match(name)
    ));
}

#[test]
fn classify_fixture_or_step_keeps_explicit_from_name_exact() {
    let mut extracted = ExtractedArgs::default();
    let mut placeholders = HashSet::new();
    let mut arg: syn::PatType = parse_quote!(state: usize);
    arg.attrs.push(parse_quote!(#[from(_world)]));
    let ty: syn::Type = parse_quote!(usize);
    let mut ctx = ClassificationContext::new(&mut extracted, &mut placeholders);
    let handled = match classify_fixture_or_step(&mut ctx, &mut arg, ident("state"), ty) {
        Ok(handled) => handled,
        Err(err) => panic!("classification should succeed: {err}"),
    };

    assert!(handled);
    assert!(matches!(
        extracted.args.as_slice(),
        [Arg::Fixture { pat, name, .. }]
        if pat == &ident("state") && name == &ident("_world")
    ));
}
