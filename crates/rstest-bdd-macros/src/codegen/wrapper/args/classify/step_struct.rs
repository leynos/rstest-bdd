//! Step-struct classifier helpers keep the main classifier module small.

use std::collections::HashSet;

use quote::ToTokens;

use super::{Arg, ExtractedArgs};

pub(crate) fn extract_step_struct_attribute(arg: &mut syn::PatType) -> syn::Result<bool> {
    super::extract_flag_attribute(arg, "step_args")
}

fn validate_condition(
    condition: bool,
    span_source: &impl ToTokens,
    error_message: &str,
) -> syn::Result<()> {
    if condition {
        Err(syn::Error::new_spanned(span_source, error_message))
    } else {
        Ok(())
    }
}

fn validate_single_step_struct(st: &ExtractedArgs, arg: &syn::PatType) -> syn::Result<()> {
    validate_condition(
        st.step_struct_idx.is_some(),
        arg,
        "only one #[step_args] parameter is permitted per step",
    )
}

fn validate_no_named_args(st: &ExtractedArgs, arg: &syn::PatType) -> syn::Result<()> {
    validate_condition(
        st.step_args().next().is_some(),
        arg,
        "#[step_args] cannot be combined with named step arguments",
    )
}

fn validate_has_placeholders(
    placeholders: &HashSet<String>,
    arg: &syn::PatType,
) -> syn::Result<()> {
    validate_condition(
        placeholders.is_empty(),
        arg,
        "#[step_args] requires at least one placeholder in the pattern",
    )
}

fn validate_no_from_attr(arg: &syn::PatType) -> syn::Result<()> {
    validate_condition(
        arg.attrs.iter().any(|a| a.path().is_ident("from")),
        arg,
        "#[step_args] cannot be combined with #[from]",
    )
}

fn validate_owned_type(ty: &syn::Type) -> syn::Result<()> {
    validate_condition(
        matches!(ty, syn::Type::Reference(_)),
        ty,
        "#[step_args] parameters must own their struct type",
    )
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
