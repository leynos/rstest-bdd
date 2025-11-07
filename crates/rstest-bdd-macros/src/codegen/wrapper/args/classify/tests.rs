//! Unit tests for the argument classifier helpers.

use super::*;
use proc_macro2::{Span, TokenStream as TokenStream2};
use quote::quote;
use std::collections::HashSet;
use syn::{parse_quote, FnArg};

fn ident(name: &str) -> syn::Ident {
    syn::Ident::new(name, Span::call_site())
}

fn pat_type(tokens: TokenStream2) -> syn::PatType {
    match syn::parse2::<FnArg>(tokens) {
        Ok(FnArg::Typed(arg)) => arg,
        Ok(FnArg::Receiver(_)) => panic!("expected typed argument"),
        Err(err) => panic!("failed to parse argument: {err}"),
    }
}

#[test]
fn context_new_links_borrows() {
    let mut extracted = ExtractedArgs::default();
    let mut placeholders = HashSet::from(["alpha".to_string()]);
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
    let mut extracted = ExtractedArgs::default();
    let mut placeholders = HashSet::from(["value".to_string()]);
    let mut arg: syn::PatType = parse_quote!(value: String);
    let pat = ident("value");
    let ty: syn::Type = parse_quote!(String);
    let handled;
    {
        let mut ctx = ClassificationContext::new(&mut extracted, &mut placeholders);
        handled = match classify_fixture_or_step(&mut ctx, &mut arg, pat, ty) {
            Ok(value) => value,
            Err(err) => panic!("classification should succeed: {err}"),
        };
    }

    assert!(handled);
    assert!(placeholders.is_empty());
    assert!(matches!(extracted.args.as_slice(), [Arg::Step { .. }]));
}

#[test]
fn classify_fixture_or_step_falls_back_to_fixture() {
    let mut extracted = ExtractedArgs::default();
    let mut placeholders = HashSet::new();
    let mut arg: syn::PatType = parse_quote!(dep: usize);
    let pat = ident("dep");
    let ty: syn::Type = parse_quote!(usize);
    let handled;
    {
        let mut ctx = ClassificationContext::new(&mut extracted, &mut placeholders);
        handled = match classify_fixture_or_step(&mut ctx, &mut arg, pat.clone(), ty) {
            Ok(value) => value,
            Err(err) => panic!("classification should succeed: {err}"),
        };
    }

    assert!(handled);
    assert!(
        matches!(extracted.args.as_slice(), [Arg::Fixture { pat: fixture_pat, .. }] if fixture_pat == &pat)
    );
}

#[test]
fn classify_fixture_or_step_respects_blocked_placeholders() {
    let mut extracted = ExtractedArgs::default();
    let idx = extracted.push(Arg::StepStruct {
        pat: ident("args"),
        ty: parse_quote!(Args),
    });
    extracted.step_struct_idx = Some(idx);
    extracted.blocked_placeholders.insert("blocked".into());
    let mut placeholders = HashSet::new();
    let mut arg: syn::PatType = parse_quote!(blocked: String);
    let pat = ident("blocked");
    let ty: syn::Type = parse_quote!(String);
    let mut ctx = ClassificationContext::new(&mut extracted, &mut placeholders);
    let Err(err) = classify_fixture_or_step(&mut ctx, &mut arg, pat, ty) else {
        panic!("classification should fail");
    };

    assert!(err
        .to_string()
        .contains("#[step_args] cannot be combined with named step arguments"));
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
    let mut placeholders = HashSet::from(["alpha".to_string(), "beta".to_string()]);
    let arg = pat_type(quote!(#[step_args] args: Args));

    match classify_step_struct(&mut extracted, &arg, &mut placeholders) {
        Ok(()) => {}
        Err(err) => panic!("step struct classification should succeed: {err}"),
    }

    assert!(placeholders.is_empty());
    assert_eq!(
        extracted.blocked_placeholders,
        HashSet::from(["alpha".to_string(), "beta".to_string()])
    );
    assert!(extracted
        .step_struct()
        .is_some_and(|step| step.pat == "args"));
}
