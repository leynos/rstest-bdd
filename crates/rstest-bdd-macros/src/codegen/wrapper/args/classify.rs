//! Classifier helpers assign each function parameter to an [`Arg`] variant.
//!
//! The `extract_args` pipeline runs these classifiers in order until one claims
//! ownership of a parameter, ensuring future extensions only need to append a
//! new function to the list rather than editing the control flow.

use std::collections::HashSet;

use super::{Arg, ExtractedArgs};

fn is_type_seq(ty: &syn::Type, seq: &[&str]) -> bool {
    use syn::{GenericArgument, PathArguments, Type};

    let mut cur = ty;
    for (i, &name) in seq.iter().enumerate() {
        let Type::Path(tp) = cur else { return false };
        let Some(segment) = tp.path.segments.last() else {
            return false;
        };
        if segment.ident != name {
            return false;
        }
        match &segment.arguments {
            PathArguments::AngleBracketed(ab) if !ab.args.is_empty() => {
                if let Some(GenericArgument::Type(inner)) = ab.args.get(0) {
                    cur = inner;
                    continue;
                }
                return false;
            }
            _ => {
                if i + 1 != seq.len() {
                    return false;
                }
            }
        }
    }
    true
}

fn is_string(ty: &syn::Type) -> bool {
    is_type_seq(ty, &["String"])
}

fn is_datatable(ty: &syn::Type) -> bool {
    is_type_seq(ty, &["Vec", "Vec", "String"])
}

fn should_classify_as_datatable(pat: &syn::Ident, ty: &syn::Type) -> bool {
    pat == "datatable" && is_datatable(ty)
}

fn extract_simple_attribute(arg: &mut syn::PatType, attr_name: &str) -> syn::Result<bool> {
    let mut found = false;
    let mut duplicate = false;
    let mut err_attr: Option<syn::Attribute> = None;
    arg.attrs.retain(|a| {
        if a.path().is_ident(attr_name) {
            if found {
                duplicate = true;
            }
            found = true;
            if a.meta.require_path_only().is_err() {
                err_attr = Some(a.clone());
            }
            false
        } else {
            true
        }
    });
    if let Some(attr) = err_attr {
        return Err(syn::Error::new_spanned(
            attr,
            format!("`#[{}]` does not take arguments", attr_name),
        ));
    }
    if duplicate {
        return Err(syn::Error::new_spanned(
            &arg.pat,
            format!("duplicate `#[{}]` attribute", attr_name),
        ));
    }
    Ok(found)
}

fn extract_datatable_attribute(arg: &mut syn::PatType) -> syn::Result<bool> {
    extract_simple_attribute(arg, "datatable")
}

fn validate_datatable_constraints(
    st: &ExtractedArgs,
    arg: &mut syn::PatType,
    pat: &syn::Ident,
    ty: &syn::Type,
) -> syn::Result<bool> {
    let is_attr = extract_datatable_attribute(arg)?;
    let is_canonical = should_classify_as_datatable(pat, ty);

    if is_attr && pat == "docstring" {
        return Err(syn::Error::new_spanned(
            arg,
            "parameter `docstring` cannot be annotated with #[datatable]",
        ));
    }
    if is_attr || is_canonical {
        if st.datatable_idx.is_some() {
            return Err(syn::Error::new_spanned(
                arg,
                "only one DataTable parameter is permitted",
            ));
        }
        if st.docstring_idx.is_some() {
            return Err(syn::Error::new_spanned(
                arg,
                "DataTable must be declared before DocString to match Gherkin ordering",
            ));
        }
        Ok(true)
    } else if pat == "datatable" {
        Err(syn::Error::new_spanned(
            arg,
            concat!(
                "parameter named `datatable` must have type `Vec<Vec<String>>` ",
                "(or use `#[datatable]` with a type that implements `TryFrom<Vec<Vec<String>>>`)",
            ),
        ))
    } else {
        Ok(false)
    }
}

pub(super) fn classify_datatable(
    st: &mut ExtractedArgs,
    arg: &mut syn::PatType,
    pat: &syn::Ident,
    ty: &syn::Type,
) -> syn::Result<bool> {
    let has_from = arg.attrs.iter().any(|a| a.path().is_ident("from"));
    let is_datatable = validate_datatable_constraints(st, arg, pat, ty)?;
    if has_from && is_datatable {
        return Err(syn::Error::new_spanned(
            arg,
            "#[datatable] cannot be combined with #[from]",
        ));
    }
    if is_datatable {
        let idx = st.push(Arg::DataTable {
            pat: pat.clone(),
            ty: ty.clone(),
        });
        st.datatable_idx = Some(idx);
        Ok(true)
    } else {
        Ok(false)
    }
}

fn is_valid_docstring_arg(st: &ExtractedArgs, pat: &syn::Ident, ty: &syn::Type) -> bool {
    st.docstring_idx.is_none() && pat == "docstring" && is_string(ty)
}

pub(super) fn classify_docstring(
    st: &mut ExtractedArgs,
    arg: &mut syn::PatType,
    pat: &syn::Ident,
    ty: &syn::Type,
) -> syn::Result<bool> {
    if is_valid_docstring_arg(st, pat, ty) {
        let idx = st.push(Arg::DocString { pat: pat.clone() });
        st.docstring_idx = Some(idx);
        Ok(true)
    } else if pat == "docstring" {
        Err(syn::Error::new_spanned(
            arg,
            "only one docstring parameter is permitted and it must have type `String`",
        ))
    } else {
        Ok(false)
    }
}

pub(super) fn extract_step_struct_attribute(arg: &mut syn::PatType) -> syn::Result<bool> {
    extract_simple_attribute(arg, "step_args")
}

pub(super) fn classify_step_struct(
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
    if st.step_struct_idx.is_some() {
        return Err(syn::Error::new_spanned(
            arg,
            "only one #[step_args] parameter is permitted per step",
        ));
    }
    if st.step_args().next().is_some() {
        return Err(syn::Error::new_spanned(
            arg,
            "#[step_args] cannot be combined with named step arguments",
        ));
    }
    if placeholders.is_empty() {
        return Err(syn::Error::new_spanned(
            arg,
            "#[step_args] requires at least one placeholder in the pattern",
        ));
    }
    if arg.attrs.iter().any(|a| a.path().is_ident("from")) {
        return Err(syn::Error::new_spanned(
            arg,
            "#[step_args] cannot be combined with #[from]",
        ));
    }
    if matches!(ty.as_ref(), syn::Type::Reference(_)) {
        return Err(syn::Error::new_spanned(
            ty.as_ref(),
            "#[step_args] parameters must own their struct type",
        ));
    }
    let idx = st.push(Arg::StepStruct {
        pat: pat.clone(),
        ty: ty.as_ref().clone(),
    });
    st.step_struct_idx = Some(idx);
    st.blocked_placeholders.clone_from(placeholders);
    placeholders.clear();
    Ok(())
}

pub(super) fn classify_fixture_or_step(
    st: &mut ExtractedArgs,
    arg: &mut syn::PatType,
    pat: syn::Ident,
    ty: syn::Type,
    placeholders: &mut HashSet<String>,
) -> syn::Result<bool> {
    let mut from_name = None;
    arg.attrs.retain(|a| {
        if a.path().is_ident("from") {
            from_name = a.parse_args().ok();
            false
        } else {
            true
        }
    });

    let target = from_name.clone().unwrap_or_else(|| pat.clone());
    let target_name = target.to_string();
    if placeholders.remove(&target_name) {
        if st.step_struct_idx.is_some() && st.blocked_placeholders.contains(&target_name) {
            return Err(syn::Error::new(
                pat.span(),
                "#[step_args] cannot be combined with named step arguments",
            ));
        }
        st.push(Arg::Step { pat, ty });
        Ok(true)
    } else if st.step_struct_idx.is_some() && st.blocked_placeholders.contains(&target_name) {
        Err(syn::Error::new(
            pat.span(),
            "#[step_args] cannot be combined with named step arguments",
        ))
    } else {
        let name = from_name.unwrap_or(target);
        st.push(Arg::Fixture { pat, name, ty });
        Ok(true)
    }
}
