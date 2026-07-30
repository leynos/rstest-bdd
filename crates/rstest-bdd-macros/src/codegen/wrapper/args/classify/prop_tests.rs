//! Property tests for the terminal classifier's naming and `#[from]` contracts.
//!
//! The unit tests in [`super::tests`] pin specific parameters and the exact
//! diagnostic text; these cover the same rules across a generated range of
//! identifiers and attribute combinations. Both are kept: the examples document
//! intent and anchor the diagnostic snapshots, while the properties catch cases
//! nobody thought to enumerate.
//!
//! Generation is deliberately bounded to syntactically parseable parameters —
//! identifiers are composed from a safe alphabet and attributes from a fixed set
//! of spellings, rather than fuzzing raw token strings, which would spend nearly
//! every case on inputs `syn` rejects before the classifier is reached.

use std::collections::HashSet;

use proc_macro2::{Span, TokenStream as TokenStream2};
use proptest::prelude::*;
use quote::quote;

use super::*;

/// First character of a generated identifier base.
///
/// Restricted to letters so a base is never empty, never starts with a digit,
/// and never degenerates to a bare `_`.
const HEAD_CHARS: [char; 5] = ['a', 'b', 'm', 'q', 'z'];

/// Subsequent characters of a generated identifier base.
///
/// Chosen so no combination with [`HEAD_CHARS`] can spell a Rust keyword; the
/// filter in [`ident_base`] is a safety net rather than the primary defence,
/// which keeps the strategy's rejection rate at effectively zero.
const TAIL_CHARS: [char; 10] = ['a', 'b', 'c', 'x', 'y', 'z', '0', '1', '9', '_'];

/// Strategy yielding a valid, non-keyword Rust identifier with no leading
/// underscore.
///
/// Leading underscores are applied separately by the callers that exercise
/// normalization, so this strategy owns only the "base" part of a name.
fn ident_base() -> impl Strategy<Value = String> {
    (
        prop::sample::select(HEAD_CHARS.as_slice()),
        prop::collection::vec(prop::sample::select(TAIL_CHARS.as_slice()), 0..6),
    )
        .prop_map(|(head, tail)| {
            let mut base = String::from(head);
            base.extend(tail);
            base
        })
        .prop_filter("must parse as a non-keyword identifier", |base| {
            syn::parse_str::<syn::Ident>(base).is_ok()
        })
}

/// Strategy yielding a parameter name together with its underscore count.
///
/// Produces `(name, underscores, base)` where `name` is `underscores` copies of
/// `_` followed by `base`. Three is enough to cover the contract's boundaries:
/// none, exactly one (stripped), and more than one (only the first stripped).
fn prefixed_name() -> impl Strategy<Value = (String, usize, String)> {
    (ident_base(), 0usize..=3).prop_map(|(base, underscores)| {
        let name = format!("{}{base}", "_".repeat(underscores));
        (name, underscores, base)
    })
}

/// Which well-formed `#[from]` spelling to emit.
#[derive(Debug, Clone)]
enum FromForm {
    /// Bare `#[from]`, which carries no name and so falls back to the
    /// normalized parameter name.
    Bare,
    /// `#[from(name)]`, naming the fixture explicitly and exactly.
    Named(String),
}

impl FromForm {
    /// Render this form as parameter attribute tokens.
    fn tokens(&self) -> TokenStream2 {
        match self {
            Self::Bare => quote!(#[from]),
            Self::Named(name) => {
                let ident = syn::Ident::new(name, Span::call_site());
                quote!(#[from(#ident)])
            }
        }
    }
}

/// Strategy yielding either well-formed `#[from]` spelling.
fn from_form() -> impl Strategy<Value = FromForm> {
    prop_oneof![Just(FromForm::Bare), ident_base().prop_map(FromForm::Named)]
}

/// Everything observable after one terminal-classifier run.
struct Outcome {
    /// The classifier's own result: `Ok(true)` on every accepted parameter.
    result: syn::Result<bool>,
    /// Arguments accumulated during the run.
    extracted: ExtractedArgs,
    /// Placeholders still unclaimed once the run finished.
    placeholders: HashSet<String>,
    /// The parameter's attributes *after* the run, for asserting what was
    /// stripped and what survived.
    attrs: Vec<syn::Attribute>,
}

impl Outcome {
    /// The error message, or the empty string when the run succeeded.
    fn error(&self) -> String {
        match &self.result {
            Ok(_) => String::new(),
            Err(err) => err.to_string(),
        }
    }

    /// Name of the sole classified fixture, if exactly one fixture was recorded.
    fn sole_fixture_name(&self) -> Option<String> {
        match self.extracted.args.as_slice() {
            [Arg::Fixture { name, .. }] => Some(name.to_string()),
            _ => None,
        }
    }

    /// Whether exactly one step argument was recorded.
    fn is_sole_step(&self) -> bool {
        matches!(self.extracted.args.as_slice(), [Arg::Step { .. }])
    }

    /// Whether any `#[from]` attribute survived the run.
    ///
    /// Generated wrapper code re-emits whatever attributes remain, so a
    /// surviving `#[from]` would reach the compiler in a position it rejects.
    fn retains_from_attribute(&self) -> bool {
        self.attrs.iter().any(|a| a.path().is_ident("from"))
    }
}

/// Build `<attrs> <name>: usize` and run the terminal classifier over it.
///
/// Panics rather than returning an error when the generated tokens do not parse:
/// that indicates a defective strategy, not a property failure, and should fail
/// loudly instead of being reported as a falsified invariant.
fn classify(attrs: &[TokenStream2], name: &str, placeholders: HashSet<String>) -> Outcome {
    let attr_tokens: TokenStream2 = attrs.iter().cloned().collect();
    let name_ident = syn::Ident::new(name, Span::call_site());
    let mut arg = match syn::parse2::<syn::FnArg>(quote!(#attr_tokens #name_ident: usize)) {
        Ok(syn::FnArg::Typed(arg)) => arg,
        Ok(syn::FnArg::Receiver(_)) => panic!("generated a receiver, expected a typed parameter"),
        Err(err) => panic!("generated parameter did not parse: {err}"),
    };
    let syn::Pat::Ident(pat_ident) = arg.pat.as_ref() else {
        panic!("generated parameter was not an identifier pattern");
    };
    let pat = pat_ident.ident.clone();
    let ty = arg.ty.as_ref().clone();

    let mut extracted = ExtractedArgs::default();
    let mut remaining = placeholders;
    let result = {
        let mut ctx = ClassificationContext::new(&mut extracted, &mut remaining);
        classify_fixture_or_step(&mut ctx, &mut arg, pat, ty)
    };

    Outcome {
        result,
        extracted,
        placeholders: remaining,
        attrs: arg.attrs,
    }
}

proptest! {
    /// Normalization strips exactly one leading underscore, never more.
    #[test]
    fn normalization_strips_exactly_one_leading_underscore(
        (name, underscores, base) in prefixed_name(),
    ) {
        let expected = format!("{}{base}", "_".repeat(underscores.saturating_sub(1)));
        prop_assert_eq!(normalize_param_name(&name), expected.as_str());
    }

    /// A parameter binds and consumes the placeholder equal to its normalized
    /// name.
    #[test]
    fn normalized_name_consumes_an_exactly_matching_placeholder(
        (name, ..) in prefixed_name(),
    ) {
        let normalized = normalize_param_name(&name).to_string();
        let outcome = classify(&[], &name, HashSet::from([normalized]));

        prop_assert_eq!(outcome.result.as_ref().ok(), Some(&true), "{}", outcome.error());
        prop_assert!(outcome.is_sole_step());
        prop_assert!(outcome.placeholders.is_empty());
    }

    /// A placeholder that does not equal the normalized name is left alone and
    /// the parameter becomes a fixture instead.
    #[test]
    fn nonmatching_placeholder_survives_and_parameter_becomes_a_fixture(
        (name, ..) in prefixed_name(),
        other in ident_base(),
    ) {
        let normalized = normalize_param_name(&name).to_string();
        prop_assume!(other != normalized);

        let outcome = classify(&[], &name, HashSet::from([other.clone()]));

        prop_assert_eq!(outcome.result.as_ref().ok(), Some(&true), "{}", outcome.error());
        prop_assert_eq!(outcome.sole_fixture_name(), Some(normalized));
        prop_assert!(outcome.placeholders.contains(&other));
    }

    /// The implicit fixture key is the normalized name, and is always a valid
    /// identifier whenever normalization succeeded.
    #[test]
    fn implicit_fixture_key_is_the_normalized_name_and_a_valid_identifier(
        (name, ..) in prefixed_name(),
    ) {
        let normalized = normalize_param_name(&name).to_string();
        let outcome = classify(&[], &name, HashSet::new());

        let Some(key) = outcome.sole_fixture_name() else {
            return Err(TestCaseError::fail("expected a single fixture argument"));
        };
        prop_assert_eq!(&key, &normalized);
        prop_assert!(syn::parse_str::<syn::Ident>(&key).is_ok(), "key `{}` is not an identifier", key);
    }

    /// A single bare `#[from]` is accepted and behaves as if omitted: the
    /// fixture key is the normalized parameter name.
    #[test]
    fn a_single_bare_from_is_accepted_and_uses_the_normalized_name(
        (name, ..) in prefixed_name(),
    ) {
        let normalized = normalize_param_name(&name).to_string();
        let outcome = classify(&[FromForm::Bare.tokens()], &name, HashSet::new());

        prop_assert_eq!(outcome.result.as_ref().ok(), Some(&true), "{}", outcome.error());
        prop_assert_eq!(outcome.sole_fixture_name(), Some(normalized));
        prop_assert!(!outcome.retains_from_attribute());
    }

    /// A single `#[from(name)]` is accepted and the named fixture key is used
    /// exactly, without normalization.
    #[test]
    fn a_single_named_from_is_accepted_and_kept_exact(
        (name, ..) in prefixed_name(),
        (explicit, ..) in prefixed_name(),
    ) {
        let outcome = classify(
            &[FromForm::Named(explicit.clone()).tokens()],
            &name,
            HashSet::new(),
        );

        prop_assert_eq!(outcome.result.as_ref().ok(), Some(&true), "{}", outcome.error());
        prop_assert_eq!(outcome.sole_fixture_name(), Some(explicit));
        prop_assert!(!outcome.retains_from_attribute());
    }

    /// Any two or more `#[from]` attributes are rejected as duplicates,
    /// whatever mix and order of bare and named spellings is used.
    #[test]
    fn any_two_from_attributes_are_rejected_as_duplicates(
        (name, ..) in prefixed_name(),
        forms in prop::collection::vec(from_form(), 2..=3),
    ) {
        let attrs: Vec<TokenStream2> = forms.iter().map(FromForm::tokens).collect();
        let outcome = classify(&attrs, &name, HashSet::new());

        prop_assert!(outcome.result.is_err(), "expected a duplicate diagnostic");
        prop_assert!(
            outcome.error().contains("duplicate `#[from]` attribute"),
            "unexpected diagnostic: {}",
            outcome.error(),
        );
    }

    /// The `#[from = ...]` name-value form is rejected as malformed.
    #[test]
    fn name_value_from_is_rejected_as_malformed(
        (name, ..) in prefixed_name(),
        value in prop_oneof![
            Just(quote!("text")),
            Just(quote!(7)),
            Just(quote!(some_path)),
        ],
    ) {
        let outcome = classify(&[quote!(#[from = #value])], &name, HashSet::new());

        prop_assert!(outcome.result.is_err(), "expected a malformed-form diagnostic");
        prop_assert!(
            outcome.error().contains("#[from] expects an identifier or no arguments"),
            "unexpected diagnostic: {}",
            outcome.error(),
        );
    }

    /// Successful classification strips the consumed `#[from]` and nothing else,
    /// leaving unrelated attributes in their original order.
    #[test]
    fn extraction_strips_only_the_consumed_from_attribute(
        (name, ..) in prefixed_name(),
        form in from_form(),
        position in 0usize..=2,
    ) {
        // Two unrelated attributes that must survive untouched, with the
        // `#[from]` threaded in before, between, or after them.
        let unrelated = [quote!(#[allow(unused)]), quote!(#[doc = "kept"])];
        let mut attrs: Vec<TokenStream2> = unrelated.to_vec();
        attrs.insert(position, form.tokens());

        let outcome = classify(&attrs, &name, HashSet::new());

        prop_assert_eq!(outcome.result.as_ref().ok(), Some(&true), "{}", outcome.error());
        prop_assert!(!outcome.retains_from_attribute());

        // A slice pattern rather than indexing, so the arity is asserted and the
        // two survivors are named in one step.
        let [first, second] = outcome.attrs.as_slice() else {
            return Err(TestCaseError::fail(format!(
                "expected the {} unrelated attributes to survive, found {}",
                unrelated.len(),
                outcome.attrs.len(),
            )));
        };
        prop_assert!(first.path().is_ident("allow"));
        prop_assert!(second.path().is_ident("doc"));
    }
}
