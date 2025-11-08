//! Step-struct classifier helpers keep the main classifier module small.

use std::collections::HashSet;

use super::{Arg, ExtractedArgs};

pub(crate) fn extract_step_struct_attribute(arg: &mut syn::PatType) -> syn::Result<bool> {
    super::extract_flag_attribute(arg, "step_args")
}

fn validate_single_step_struct(st: &ExtractedArgs, span: &syn::PatType) -> syn::Result<()> {
    if st.step_struct_idx.is_some() {
        Err(syn::Error::new_spanned(
            span,
            "only one #[step_args] parameter is permitted per step",
        ))
    } else {
        Ok(())
    }
}

fn validate_no_named_args(st: &ExtractedArgs, span: &syn::PatType) -> syn::Result<()> {
    if st.step_args().next().is_some() {
        Err(syn::Error::new_spanned(
            span,
            "#[step_args] cannot be combined with named step arguments",
        ))
    } else {
        Ok(())
    }
}

fn validate_has_placeholders(
    placeholders: &HashSet<String>,
    span: &syn::PatType,
) -> syn::Result<()> {
    if placeholders.is_empty() {
        Err(syn::Error::new_spanned(
            span,
            "#[step_args] requires at least one placeholder in the pattern",
        ))
    } else {
        Ok(())
    }
}

fn validate_no_from_attr(arg: &syn::PatType) -> syn::Result<()> {
    if arg.attrs.iter().any(|a| a.path().is_ident("from")) {
        Err(syn::Error::new_spanned(
            arg,
            "#[step_args] cannot be combined with #[from]",
        ))
    } else {
        Ok(())
    }
}

fn validate_owned_type(ty: &syn::Type) -> syn::Result<()> {
    if matches!(ty, syn::Type::Reference(_)) {
        Err(syn::Error::new_spanned(
            ty,
            "#[step_args] parameters must own their struct type",
        ))
    } else {
        Ok(())
    }
}

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
    validate_has_placeholders(placeholders, arg)?;
    validate_no_from_attr(arg)?;
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
    use super::*;
    use proc_macro2::{Span, TokenStream as TokenStream2};
    use quote::quote;
    use syn::{parse_quote, FnArg, Ident};

    fn placeholder_set(names: &[&str]) -> HashSet<String> {
        names.iter().map(|name| (*name).to_string()).collect()
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
