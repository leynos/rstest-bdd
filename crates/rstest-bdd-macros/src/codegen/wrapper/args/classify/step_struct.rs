//! Step-struct classifier helpers keep the main classifier module small.

use std::collections::HashSet;

use super::{Arg, ExtractedArgs};

pub(crate) fn extract_step_struct_attribute(
    arg: &mut syn::PatType,
) -> syn::Result<bool> {
    super::extract_flag_attribute(arg, "step_args")
}

fn validate_single_step_struct(st: &ExtractedArgs, arg: &syn::PatType) -> syn::Result<()> {
    if st.step_struct_idx.is_some() {
        Err(syn::Error::new_spanned(
            arg,
            "only one #[step_args] parameter is permitted per step",
        ))
    } else {
        Ok(())
    }
}

fn validate_no_named_args(st: &ExtractedArgs, arg: &syn::PatType) -> syn::Result<()> {
    if st.step_args().next().is_some() {
        Err(syn::Error::new_spanned(
            arg,
            "#[step_args] cannot be combined with named step arguments",
        ))
    } else {
        Ok(())
    }
}

fn validate_has_placeholders(
    placeholders: &HashSet<String>,
    arg: &syn::PatType,
) -> syn::Result<()> {
    if placeholders.is_empty() {
        Err(syn::Error::new_spanned(
            arg,
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
