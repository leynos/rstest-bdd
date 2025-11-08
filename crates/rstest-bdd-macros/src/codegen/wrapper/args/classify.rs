//! Classifier helpers assign each function parameter to an [`Arg`] variant.
//!
//! The `extract_args` pipeline runs these classifiers in order until one claims
//! ownership of a parameter, ensuring future extensions only need to append a
//! new function to the list rather than editing the control flow. Attribute
//! validation lives here so the pipeline can provide precise diagnostics while
//! keeping the orchestration layer slim.

use std::collections::HashSet;

use super::{Arg, ExtractedArgs};

mod step_struct;

pub(super) use step_struct::{classify_step_struct, extract_step_struct_attribute};

const DATATABLE_TYPE_ERROR: &str = concat!(
    "parameter named `datatable` must have type `Vec<Vec<String>>` ",
    "(or use `#[datatable]` with a type that implements `TryFrom<Vec<Vec<String>>>`)",
);
const DOCSTRING_TYPE_ERROR: &str =
    "only one docstring parameter is permitted and it must have type `String`";

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

pub(super) fn extract_flag_attribute(arg: &mut syn::PatType, attr_name: &str) -> syn::Result<bool> {
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

struct FlagMatchConfig<F>
where
    F: Fn(&syn::Ident, &syn::Type) -> bool,
{
    attr_name: Option<&'static str>,
    canonical_name: &'static str,
    canonical_check: F,
    wrong_type_msg: &'static str,
}

impl<F> FlagMatchConfig<F>
where
    F: Fn(&syn::Ident, &syn::Type) -> bool,
{
    fn new(
        attr_name: Option<&'static str>,
        canonical_name: &'static str,
        canonical_check: F,
        wrong_type_msg: &'static str,
    ) -> Self {
        Self {
            attr_name,
            canonical_name,
            canonical_check,
            wrong_type_msg,
        }
    }
}

fn match_named_flag<F>(
    arg: &mut syn::PatType,
    pat: &syn::Ident,
    ty: &syn::Type,
    config: FlagMatchConfig<F>,
) -> syn::Result<Option<FlagMatch>>
where
    F: Fn(&syn::Ident, &syn::Type) -> bool,
{
    let FlagMatchConfig {
        attr_name,
        canonical_name,
        canonical_check,
        wrong_type_msg,
    } = config;

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
        FlagMatchConfig::new(
            Some("datatable"),
            "datatable",
            should_classify_as_datatable,
            DATATABLE_TYPE_ERROR,
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
        FlagMatchConfig::new(
            None,
            "docstring",
            is_docstring_canonical,
            DOCSTRING_TYPE_ERROR,
        ),
    )?;
    let Some(_) = match_result else {
        return Ok(false);
    };
    if st.docstring_idx.is_some() {
        return Err(syn::Error::new_spanned(arg, DOCSTRING_TYPE_ERROR));
    }
    let idx = st.push(Arg::DocString { pat: pat.clone() });
    st.docstring_idx = Some(idx);
    Ok(true)
}

fn parse_from_attribute(arg: &mut syn::PatType) -> syn::Result<Option<syn::Ident>> {
    let mut from_name = None;
    let mut from_attr_err = None;
    arg.attrs.retain(|a| {
        if !a.path().is_ident("from") {
            return true;
        }
        if from_attr_err.is_some() {
            return false;
        }
        match &a.meta {
            syn::Meta::Path(_) => {}
            syn::Meta::List(_) => match a.parse_args::<syn::Ident>() {
                Ok(parsed) => from_name = Some(parsed),
                Err(err) => from_attr_err = Some(err),
            },
            syn::Meta::NameValue(_) => {
                from_attr_err = Some(syn::Error::new_spanned(
                    a,
                    "#[from] expects an identifier or no arguments",
                ));
            }
        }
        false
    });
    if let Some(err) = from_attr_err {
        return Err(err);
    }
    Ok(from_name)
}

fn validate_no_step_struct_conflict(
    ctx: &ClassificationContext,
    target_name: &str,
    pat: &syn::Ident,
) -> syn::Result<()> {
    if ctx.extracted.step_struct_idx.is_some()
        && ctx.extracted.blocked_placeholders.contains(target_name)
    {
        Err(syn::Error::new(
            pat.span(),
            "#[step_args] cannot be combined with named step arguments",
        ))
    } else {
        Ok(())
    }
}

fn classify_by_placeholder_match(
    ctx: &mut ClassificationContext,
    from_name: Option<syn::Ident>,
    pat: syn::Ident,
    ty: syn::Type,
) -> syn::Result<()> {
    let target = from_name.clone().unwrap_or_else(|| pat.clone());
    let target_name = target.to_string();
    if ctx.placeholders.remove(&target_name) {
        validate_no_step_struct_conflict(ctx, &target_name, &pat)?;
        ctx.extracted.push(Arg::Step { pat, ty });
        Ok(())
    } else {
        validate_no_step_struct_conflict(ctx, &target_name, &pat)?;
        let name = from_name.unwrap_or_else(|| pat.clone());
        ctx.extracted.push(Arg::Fixture { pat, name, ty });
        Ok(())
    }
}

pub(super) fn classify_fixture_or_step(
    ctx: &mut ClassificationContext,
    arg: &mut syn::PatType,
    pat: syn::Ident,
    ty: syn::Type,
) -> syn::Result<bool> {
    let from_name = parse_from_attribute(arg)?;
    classify_by_placeholder_match(ctx, from_name, pat, ty)?;
    Ok(true)
}

#[cfg(test)]
mod tests;
