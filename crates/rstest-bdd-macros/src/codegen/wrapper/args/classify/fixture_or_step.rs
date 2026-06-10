//! Terminal classifier: step-argument (placeholder) or fixture binding.
//!
//! Runs after the DataTable/DocString/step-struct classifiers have declined a
//! parameter. Consumes any `#[from(...)]` attribute to determine the lookup
//! name, claims a matching step-pattern placeholder where possible, and
//! otherwise records the parameter as a fixture injection.

use super::ClassificationContext;
use super::{Arg, normalize_param_name};

/// Extract the fixture name from a `#[from(...)]` attribute, if present.
///
/// Mutates `arg.attrs` in place, removing every `#[from]` attribute so the
/// generated wrapper does not re-emit it. Returns the explicit fixture
/// identifier from `#[from(name)]`, or `None` for a bare `#[from]` or no
/// attribute.
///
/// # Errors
///
/// Returns an error when the attribute payload is not a single identifier or
/// uses `#[from = ...]` name-value form.
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
    let normalized = normalize_param_name(&target_name);
    if ctx.placeholders.remove(normalized) {
        validate_no_step_struct_conflict(ctx, normalized, &pat)?;
        ctx.extracted.push(Arg::Step { pat, ty });
        Ok(())
    } else {
        validate_no_step_struct_conflict(ctx, &target_name, &pat)?;
        let name = if let Some(name) = from_name {
            name
        } else if normalized == target_name {
            pat.clone()
        } else {
            let mut name = syn::parse_str::<syn::Ident>(normalized).map_err(|_| {
                syn::Error::new(
                    pat.span(),
                    format!(
                        "normalized fixture name `{normalized}` is not a valid identifier; use #[from(...)] to specify the fixture name explicitly"
                    ),
                )
            })?;
            name.set_span(pat.span());
            name
        };
        ctx.extracted.push(Arg::Fixture { pat, name, ty });
        Ok(())
    }
}

/// Classify `arg` as a step argument (placeholder match) or fixture.
///
/// Consumes any `#[from(...)]` attribute on `arg` (stripping it in place via
/// [`parse_from_attribute`]) to determine the lookup name, then claims a
/// matching placeholder from `ctx.placeholders` or falls back to fixture
/// injection. Always returns `Ok(true)`: this is the terminal classifier in
/// the pipeline.
///
/// # Errors
///
/// Returns an error when the `#[from]` attribute is malformed, when the
/// parameter conflicts with `#[step_args]` placeholder ownership, or when a
/// normalized fixture name is not a valid identifier.
pub(in super::super) fn classify_fixture_or_step(
    ctx: &mut ClassificationContext,
    arg: &mut syn::PatType,
    pat: syn::Ident,
    ty: syn::Type,
) -> syn::Result<bool> {
    let from_name = parse_from_attribute(arg)?;
    classify_by_placeholder_match(ctx, from_name, pat, ty)?;
    Ok(true)
}
