//! Classifier helpers assign each function parameter to an [`Arg`] variant.
//!
//! The `extract_args` pipeline runs these classifiers in order until one claims
//! ownership of a parameter, ensuring future extensions only need to append a
//! new function to the list rather than editing the control flow. Attribute
//! validation lives here so the pipeline can provide precise diagnostics while
//! keeping the orchestration layer slim.

use std::collections::HashSet;

use super::{Arg, ExtractedArgs};

pub(super) struct ClassificationContext<'a> {
    pub(super) extracted: &'a mut ExtractedArgs,
    pub(super) placeholders: &'a mut HashSet<String>,
}

impl<'a> ClassificationContext<'a> {
    pub(super) fn new(
        extracted: &'a mut ExtractedArgs,
        placeholders: &'a mut HashSet<String>,
    ) -> Self {
        Self {
            extracted,
            placeholders,
        }
    }
}

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

fn extract_flag_attribute(arg: &mut syn::PatType, attr_name: &str) -> syn::Result<bool> {
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
            format!("`#[{attr_name}]` does not take arguments"),
        ));
    }
    if duplicate {
        return Err(syn::Error::new_spanned(
            &arg.pat,
            format!("duplicate `#[{attr_name}]` attribute"),
        ));
    }
    Ok(found)
}

struct FlagMatch {
    via_attr: bool,
}

fn match_named_flag<F>(
    arg: &mut syn::PatType,
    pat: &syn::Ident,
    ty: &syn::Type,
    attr_name: Option<&'static str>,
    canonical_name: &'static str,
    canonical_check: F,
    wrong_type_msg: &str,
) -> syn::Result<Option<FlagMatch>>
where
    F: Fn(&syn::Ident, &syn::Type) -> bool,
{
    let via_attr = if let Some(name) = attr_name {
        extract_flag_attribute(arg, name)?
    } else {
        false
    };
    let canonical = canonical_check(pat, ty);
    if via_attr || canonical {
        Ok(Some(FlagMatch { via_attr }))
    } else if pat == canonical_name {
        Err(syn::Error::new_spanned(arg, wrong_type_msg))
    } else {
        Ok(None)
    }
}

pub(super) fn classify_datatable(
    st: &mut ExtractedArgs,
    arg: &mut syn::PatType,
    pat: &syn::Ident,
    ty: &syn::Type,
) -> syn::Result<bool> {
    let has_from = arg.attrs.iter().any(|a| a.path().is_ident("from"));
    let match_result = match_named_flag(
        arg,
        pat,
        ty,
        Some("datatable"),
        "datatable",
        should_classify_as_datatable,
        concat!(
            "parameter named `datatable` must have type `Vec<Vec<String>>` ",
            "(or use `#[datatable]` with a type that implements `TryFrom<Vec<Vec<String>>>`)",
        ),
    )?;
    let Some(flag_match) = match_result else {
        return Ok(false);
    };
    if flag_match.via_attr && pat == "docstring" {
        return Err(syn::Error::new_spanned(
            arg,
            "parameter `docstring` cannot be annotated with #[datatable]",
        ));
    }
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
    if has_from {
        return Err(syn::Error::new_spanned(
            arg,
            "#[datatable] cannot be combined with #[from]",
        ));
    }
    let idx = st.push(Arg::DataTable {
        pat: pat.clone(),
        ty: ty.clone(),
    });
    st.datatable_idx = Some(idx);
    Ok(true)
}

fn is_docstring_canonical(pat: &syn::Ident, ty: &syn::Type) -> bool {
    pat == "docstring" && is_string(ty)
}

pub(super) fn classify_docstring(
    st: &mut ExtractedArgs,
    arg: &mut syn::PatType,
    pat: &syn::Ident,
    ty: &syn::Type,
) -> syn::Result<bool> {
    let match_result = match_named_flag(
        arg,
        pat,
        ty,
        None,
        "docstring",
        is_docstring_canonical,
        "only one docstring parameter is permitted and it must have type `String`",
    )?;
    let Some(_) = match_result else {
        return Ok(false);
    };
    if st.docstring_idx.is_some() {
        return Err(syn::Error::new_spanned(
            arg,
            "only one docstring parameter is permitted and it must have type `String`",
        ));
    }
    let idx = st.push(Arg::DocString { pat: pat.clone() });
    st.docstring_idx = Some(idx);
    Ok(true)
}

pub(super) fn extract_step_struct_attribute(arg: &mut syn::PatType) -> syn::Result<bool> {
    extract_flag_attribute(arg, "step_args")
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
    ctx: &mut ClassificationContext,
    arg: &mut syn::PatType,
    pat: syn::Ident,
    ty: syn::Type,
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
    if ctx.placeholders.remove(&target_name) {
        if ctx.extracted.step_struct_idx.is_some()
            && ctx.extracted.blocked_placeholders.contains(&target_name)
        {
            return Err(syn::Error::new(
                pat.span(),
                "#[step_args] cannot be combined with named step arguments",
            ));
        }
        ctx.extracted.push(Arg::Step { pat, ty });
        Ok(true)
    } else if ctx.extracted.step_struct_idx.is_some()
        && ctx.extracted.blocked_placeholders.contains(&target_name)
    {
        Err(syn::Error::new(
            pat.span(),
            "#[step_args] cannot be combined with named step arguments",
        ))
    } else {
        let name = from_name.unwrap_or(target);
        ctx.extracted.push(Arg::Fixture { pat, name, ty });
        Ok(true)
    }
}

#[cfg(test)]
mod tests;
