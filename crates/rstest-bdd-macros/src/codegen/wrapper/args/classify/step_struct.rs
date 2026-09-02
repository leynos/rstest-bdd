//! Step-struct classifier helpers keep the main classifier module small.

use std::collections::HashSet;

use quote::ToTokens;

use super::{Arg, ExtractedArgs};

/// Detect and strip the `#[step_args]` marker from `arg`.
///
/// Returns whether the marker was present. Mutates [`syn::PatType::attrs`] in
/// place, removing the marker so the generated wrapper does not re-emit it; see
/// [`super::extract_flag_attribute`] for the shared stripping contract.
///
/// # Errors
///
/// Returns an error when `#[step_args]` carries arguments (it is a bare marker)
/// or appears more than once on the same parameter.
pub(crate) fn extract_step_struct_attribute(arg: &mut syn::PatType) -> syn::Result<bool> {
    super::extract_flag_attribute(arg, "step_args")
}

/// Turn a *rejection* predicate into a `Result`, attributing the span to `span`.
///
/// Note the polarity: `condition` describes the failure, so `true` yields
/// `Err(error_msg)` and `false` yields `Ok(())`. Each `validate_*` helper below
/// reads as "error if …" for this reason.
///
/// # Errors
///
/// Returns an error carrying `error_msg`, spanned to `span`, when `condition`
/// holds.
fn validate_condition<T>(condition: bool, span: &T, error_msg: &str) -> syn::Result<()>
where
    T: ToTokens,
{
    if condition {
        Err(syn::Error::new_spanned(span, error_msg))
    } else {
        Ok(())
    }
}

/// Reject a second `#[step_args]` parameter in one step signature.
///
/// # Errors
///
/// Returns an error when `st` already recorded a step struct.
fn validate_single_step_struct(st: &ExtractedArgs, span: &syn::PatType) -> syn::Result<()> {
    validate_condition(
        st.step_struct_idx.is_some(),
        span,
        "only one #[step_args] parameter is permitted per step",
    )
}

/// Reject a `#[step_args]` struct alongside individually named step arguments.
///
/// The struct claims every placeholder in the pattern, so a separate named
/// parameter would have nothing left to bind.
///
/// # Errors
///
/// Returns an error when `st` already recorded at least one named step argument.
fn validate_no_named_args(st: &ExtractedArgs, span: &syn::PatType) -> syn::Result<()> {
    validate_condition(
        st.step_args().next().is_some(),
        span,
        "#[step_args] cannot be combined with named step arguments",
    )
}

/// Reject a `#[step_args]` struct when the pattern has no placeholders to fill.
///
/// # Errors
///
/// Returns an error when `placeholders` is empty, since the struct would have
/// no fields to populate.
fn validate_has_placeholders(
    placeholders: &HashSet<String>,
    span: &syn::PatType,
) -> syn::Result<()> {
    validate_condition(
        placeholders.is_empty(),
        span,
        "#[step_args] requires at least one placeholder in the pattern",
    )
}

/// Reject `#[from]` on a `#[step_args]` parameter.
///
/// `#[from]` selects a fixture by name, which is meaningless for a struct built
/// from pattern placeholders. Inspects `arg.attrs` without mutating it, so the
/// attribute is still present in the diagnostic's span.
///
/// # Errors
///
/// Returns an error when `arg` carries any `#[from]` attribute.
fn validate_no_from_attr(arg: &syn::PatType) -> syn::Result<()> {
    validate_condition(
        arg.attrs.iter().any(|a| a.path().is_ident("from")),
        arg,
        "#[step_args] cannot be combined with #[from]",
    )
}

/// Reject `#[harness_context]` on a `#[step_args]` parameter.
///
/// The `#[step_args]` classifier runs before the `#[harness_context]`
/// classifier in `extract_args`, so this guard is what stops the marker from
/// surviving into generated code when the attributes appear in this order.
/// The message agrees with `classify_harness_context`'s own rejection so users
/// see one story regardless of attribute order.
///
/// # Errors
///
/// Returns an error when `arg` carries a `#[harness_context]` attribute.
fn validate_harness_context_attr(arg: &syn::PatType) -> syn::Result<()> {
    validate_condition(
        arg.attrs
            .iter()
            .any(|a| a.path().is_ident("harness_context")),
        arg,
        super::harness_context::HARNESS_CONTEXT_WITH_STEP_ARGS_ERROR,
    )
}

/// Require that a `#[step_args]` parameter owns its struct type.
///
/// The generated wrapper constructs the struct locally and moves it into the
/// step call, so a reference parameter would borrow a temporary.
///
/// # Errors
///
/// Returns an error when `ty` is a reference type.
fn validate_owned_type(ty: &syn::Type) -> syn::Result<()> {
    validate_condition(
        matches!(ty, &syn::Type::Reference(_)),
        ty,
        "#[step_args] parameters must own their struct type",
    )
}

/// Classify an already-marked `#[step_args]` parameter as the step struct.
///
/// Called only after [`extract_step_struct_attribute`] confirmed and stripped
/// the marker, so this function validates rather than detects: it runs the
/// `validate_*` guards above in order and records the struct on success. There
/// is no "not a match" outcome — the parameter is either accepted or rejected.
///
/// # Mutation contract
///
/// On success, and only on success:
///
/// - [`Arg::StepStruct`] is appended to `st` and `st.step_struct_idx` is set to its index.
/// - `st.blocked_placeholders` is overwritten with a clone of `placeholders`, recording the names
///   the struct now owns so a later parameter cannot rebind one. [`super::fixture_or_step`]
///   consults this set.
/// - `placeholders` is then **cleared**, since the struct consumes every placeholder at once.
///
/// `arg` is not mutated; the marker was already removed by the caller.
///
/// # Errors
///
/// Returns an error when the parameter is not a simple identifier pattern, when
/// a step struct was already classified, when named step arguments are also
/// present, when the pattern has no placeholders, when `#[from]` is combined
/// with `#[step_args]`, or when the parameter type is a reference. On any error
/// `st` and `placeholders` are left untouched.
pub(crate) fn classify_step_struct(
    st: &mut ExtractedArgs,
    arg: &syn::PatType,
    placeholders: &mut HashSet<String>,
) -> syn::Result<()> {
    let syn::Pat::Ident(pat_ident) = arg.pat.as_ref() else {
        return Err(syn::Error::new_spanned(
            &arg.pat,
            "#[step_args] requires a simple identifier pattern",
        ));
    };
    let pat = &pat_ident.ident;
    let ty = &arg.ty;
    validate_single_step_struct(st, arg)?;
    validate_no_named_args(st, arg)?;
    validate_no_from_attr(arg)?;
    validate_harness_context_attr(arg)?;
    validate_has_placeholders(placeholders, arg)?;
    validate_owned_type(ty.as_ref())?;
    let idx = st.push(Arg::StepStruct {
        pat: pat.clone(),
        ty: ty.as_ref().clone(),
    });
    st.step_struct_idx = Some(idx);
    st.blocked_placeholders.clone_from(placeholders);
    placeholders.clear();
    Ok(())
}

#[cfg(test)]
mod tests {
    //! Unit tests for classifying step struct parameters.

    use proc_macro2::{Span, TokenStream as TokenStream2};
    use quote::quote;
    use syn::{FnArg, Ident, parse_quote};

    use super::*;

    fn placeholder_set(names: &[&str]) -> HashSet<String> {
        names.iter().map(|name| (*name).to_owned()).collect()
    }

    fn pat(tokens: TokenStream2) -> syn::PatType {
        match syn::parse2::<FnArg>(tokens) {
            Ok(FnArg::Typed(arg)) => arg,
            Ok(FnArg::Receiver(_)) => panic!("expected typed argument"),
            Err(err) => panic!("failed to parse argument: {err}"),
        }
    }

    /// Helper to test `classify_step_struct` with various scenarios.
    fn assert_classify_step_struct(
        setup: impl FnOnce(&mut ExtractedArgs),
        placeholder_names: &[&str],
        arg_tokens: TokenStream2,
        expected_error_fragment: Option<&str>,
    ) {
        let mut extracted = ExtractedArgs::default();
        setup(&mut extracted);
        let mut placeholders = placeholder_set(placeholder_names);
        let arg = pat(arg_tokens);

        match (
            classify_step_struct(&mut extracted, &arg, &mut placeholders),
            expected_error_fragment,
        ) {
            (Ok(()), Some(expected)) => {
                panic!("classification should fail containing '{expected}'");
            }
            (Ok(()), None) => {}
            (Err(err), None) => panic!("classification should succeed: {err}"),
            (Err(err), Some(expected)) => {
                assert!(
                    err.to_string().contains(expected),
                    "error '{err}' did not contain expected fragment '{expected}'"
                );
            }
        }

        if expected_error_fragment.is_none() {
            assert!(placeholders.is_empty());
            assert!(matches!(
                extracted.args.as_slice(),
                [Arg::StepStruct { .. }]
            ));
        }
    }

    /// Setup function: adds a pre-existing step struct to create a conflict.
    fn setup_with_existing_step_struct(extracted: &mut ExtractedArgs) {
        extracted.step_struct_idx = Some(extracted.push(Arg::StepStruct {
            pat: Ident::new("existing", Span::call_site()),
            ty: parse_quote!(Args),
        }));
    }

    /// Setup function: adds a pre-existing named step argument to create a conflict.
    fn setup_with_existing_step_arg(extracted: &mut ExtractedArgs) {
        extracted.push(Arg::Step {
            pat: Ident::new("value", Span::call_site()),
            ty: parse_quote!(String),
        });
    }

    #[test]
    fn classifies_step_struct_and_clears_placeholders() {
        assert_classify_step_struct(|_| {}, &["value"], quote!(#[step_args] args: Args), None);
    }

    #[test]
    fn rejects_duplicate_step_structs() {
        assert_classify_step_struct(
            setup_with_existing_step_struct,
            &["value"],
            quote!(#[step_args] args: Args),
            Some("only one #[step_args] parameter is permitted per step"),
        );
    }

    #[test]
    fn rejects_mix_with_named_arguments() {
        assert_classify_step_struct(
            setup_with_existing_step_arg,
            &["value"],
            quote!(#[step_args] args: Args),
            Some("#[step_args] cannot be combined with named step arguments"),
        );
    }

    #[test]
    fn rejects_missing_placeholders() {
        assert_classify_step_struct(
            |_| {},
            &[],
            quote!(#[step_args] args: Args),
            Some("#[step_args] requires at least one placeholder"),
        );
    }

    #[test]
    fn rejects_with_from_attribute() {
        assert_classify_step_struct(
            |_| {},
            &["value"],
            quote!(#[step_args] #[from] args: Args),
            Some("#[step_args] cannot be combined with #[from]"),
        );
    }

    #[test]
    fn rejects_reference_types() {
        assert_classify_step_struct(
            |_| {},
            &["value"],
            quote!(#[step_args] args: &Args),
            Some("#[step_args] parameters must own their struct type"),
        );
    }
}
