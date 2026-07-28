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
/// Returns an error when the attribute payload is not a single identifier,
/// uses the `#[from = ...]` name-value form, or when more than one `#[from]`
/// attribute is applied to the same parameter (which would otherwise silently
/// pick the last one).
///
/// # Examples
///
/// Given the parameter as written in a step function:
///
/// ```text
/// #[from(user)] u: User      => Ok(Some(user)); attribute stripped
/// #[from] u: User            => Ok(None);       attribute stripped
/// u: User                    => Ok(None);       nothing to strip
/// #[from = "user"] u: U      => Err: "#[from] expects an identifier or no arguments"
/// #[from(a)] #[from(b)] u: U  => Err: "duplicate `#[from]` attribute"
/// ```
fn parse_from_attribute(arg: &mut syn::PatType) -> syn::Result<Option<syn::Ident>> {
    let mut from_name = None;
    let mut from_attr_err = None;
    let mut found = false;
    let mut duplicate = false;
    arg.attrs.retain(|a| {
        if !a.path().is_ident("from") {
            return true;
        }
        if found {
            duplicate = true;
        }
        found = true;
        // Keep scanning so every `#[from]` is stripped, but stop parsing
        // payloads once the first error (malformed or duplicate) is seen.
        if from_attr_err.is_some() || duplicate {
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
    if duplicate {
        return Err(syn::Error::new_spanned(
            &arg.pat,
            "duplicate `#[from]` attribute",
        ));
    }
    Ok(from_name)
}

/// Reject a fixture/step name that a `#[step_args]` struct already owns.
///
/// When a `#[step_args]` struct is present it claims a set of placeholder
/// names (`ctx.extracted.blocked_placeholders`); binding a separate parameter
/// to one of those names would silently shadow the struct. `target_name` must
/// be the *normalized* lookup name (leading underscore stripped) so the check
/// uses the same key space the placeholder set and the struct do — otherwise a
/// parameter such as `_foo` could slip past a block on `foo`.
///
/// Does not mutate `ctx`; it only inspects the accumulated state.
///
/// # Errors
///
/// Returns an error when a `#[step_args]` struct is present and `target_name`
/// is one of its blocked placeholders.
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

/// Resolve the fixture name for a parameter that matched no placeholder.
///
/// Prefers an explicit `#[from(name)]` override; otherwise uses the parameter
/// identifier when normalization left it unchanged, or the normalized name
/// (re-parsed as an identifier) when a leading underscore was stripped. Does
/// not mutate anything.
///
/// # Errors
///
/// Returns an error when the normalized name is not a valid identifier — for
/// example a name that becomes empty or begins with a digit after stripping —
/// directing the caller to name the fixture explicitly with `#[from(...)]`.
fn resolve_fixture_name(
    from_name: Option<syn::Ident>,
    pat: &syn::Ident,
    normalized: &str,
) -> syn::Result<syn::Ident> {
    if let Some(name) = from_name {
        return Ok(name);
    }
    if pat == normalized {
        return Ok(pat.clone());
    }
    let mut name = syn::parse_str::<syn::Ident>(normalized).map_err(|_| {
        syn::Error::new(
            pat.span(),
            format!(
                "normalized fixture name `{normalized}` is not a valid identifier; use #[from(...)] to specify the fixture name explicitly"
            ),
        )
    })?;
    name.set_span(pat.span());
    Ok(name)
}

/// Bind `pat`/`ty` to a step-pattern placeholder or, failing that, a fixture.
///
/// The lookup name is `from_name` when `#[from(...)]` was supplied, otherwise
/// the parameter identifier; either way it is normalized (leading underscore
/// stripped) before matching. On a placeholder hit the entry is removed from
/// `ctx.placeholders` and an [`Arg::Step`] is pushed; otherwise an
/// [`Arg::Fixture`] is pushed with the name from [`resolve_fixture_name`].
///
/// Mutates `ctx` in place: it removes the matched placeholder from
/// `ctx.placeholders` and appends the classified [`Arg`] to `ctx.extracted`.
///
/// # Errors
///
/// Returns an error when the normalized name collides with a `#[step_args]`
/// blocked placeholder (via [`validate_no_step_struct_conflict`]) or when a
/// normalized fixture name is not a valid identifier (via
/// [`resolve_fixture_name`]).
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
        validate_no_step_struct_conflict(ctx, normalized, &pat)?;
        let name = resolve_fixture_name(from_name, &pat, normalized)?;
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
///
/// # Examples
///
/// Given the parameter as written in a step function and the current
/// placeholder set:
///
/// ```text
/// count: u32,  placeholders = {count}  => Step arg; placeholder `count` consumed
/// pool: DbPool, placeholders = {}      => Fixture named `pool`
/// #[from(db)] pool: DbPool, {}         => Fixture named `db` (explicit override)
/// ```
///
/// Always returns `Ok(true)`; the parameter is claimed either way.
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
