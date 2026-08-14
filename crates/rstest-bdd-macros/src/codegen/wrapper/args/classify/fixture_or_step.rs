//! Terminal classifier: step-argument (placeholder) or fixture binding.
//!
//! Runs after the DataTable/DocString/step-struct classifiers have declined a
//! parameter. Consumes any `#[from(...)]` attribute to determine the lookup
//! name, claims a matching step-pattern placeholder where possible, and
//! otherwise records the parameter as a fixture injection.

use super::{Arg, ClassificationContext, normalize_param_name};

/// Extract the fixture name from a `#[from(...)]` attribute, if present.
///
/// Returns the explicit fixture identifier from `#[from(name)]`, or `None` for
/// a bare `#[from]` or no attribute at all.
///
/// # Mutation contract
///
/// On successful extraction the consumed `#[from(...)]` attribute is removed
/// from [`syn::PatType::attrs`] in place, so the generated wrapper does not
/// re-emit an attribute the compiler would reject in that position.
///
/// Stripping is unconditional rather than success-only: every `#[from]`
/// attribute is removed before any error is returned, because the scan that
/// removes them is also the scan that detects duplicates. A caller that
/// recovers from an error therefore sees `arg.attrs` already cleared of
/// `#[from]`, not restored to its original state. Only `#[from]` attributes are
/// touched; any other attribute on the parameter is left in place and in order.
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
        if from_attr_err.is_none() && !duplicate {
            match from_attribute_ident(a) {
                Ok(name) => from_name = name,
                Err(err) => from_attr_err = Some(err),
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

/// Extract the explicit fixture name from a single `#[from]` attribute's
/// payload.
///
/// Returns `Ok(None)` for a bare `#[from]`, `Ok(Some(name))` for
/// `#[from(name)]`.
///
/// # Errors
///
/// Returns an error when the payload is not a single identifier or uses the
/// `#[from = ...]` name-value form.
fn from_attribute_ident(attr: &syn::Attribute) -> syn::Result<Option<syn::Ident>> {
    match &attr.meta {
        syn::Meta::Path(_) => Ok(None),
        syn::Meta::List(_) => attr.parse_args::<syn::Ident>().map(Some),
        syn::Meta::NameValue(_) => Err(syn::Error::new_spanned(
            attr,
            "#[from] expects an identifier or no arguments",
        )),
    }
}

/// Reject a fixture/step name that a `#[step_args]` struct already owns.
///
/// When a `#[step_args]` struct is present it claims a set of placeholder
/// names (`ctx.extracted.blocked_placeholders`); binding a separate parameter
/// to one of those names would silently shadow the struct. `target_name` must be
/// the key the parameter will actually request — the normalized parameter name
/// for an implicit fixture, or the explicit `#[from(...)]` identifier verbatim —
/// so the check uses the same key space as the placeholder set and the struct.
/// An implicit `_foo` must therefore be tested as `foo`, or it would slip past a
/// block on `foo`; an explicit `#[from(_foo)]` must be tested as `_foo`, or it
/// would be rejected by a block it does not collide with.
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
                "normalized fixture name `{normalized}` is not a valid identifier; use \
                 #[from(...)] to specify the fixture name explicitly"
            ),
        )
    })?;
    name.set_span(pat.span());
    Ok(name)
}

/// Bind `pat`/`ty` to a step-pattern placeholder or, failing that, a fixture.
///
/// The lookup name is the explicit `#[from(...)]` identifier **verbatim** when
/// one was supplied, and otherwise the parameter identifier with one leading
/// underscore stripped. On a placeholder hit the entry is removed from
/// `ctx.placeholders` and an [`Arg::Step`] is pushed; otherwise an
/// [`Arg::Fixture`] is pushed with the name from [`resolve_fixture_name`].
///
/// Normalization applies to the implicit name only, because only that path goes
/// on to *record* a normalized key. An explicit `#[from(name)]` is authoritative
/// and records `name` exactly, so matching or guarding it against a stripped
/// form would test a key the parameter never requests: `#[from(_foo)]` would
/// bind the placeholder `foo`, discarding the explicit name, and would be
/// rejected by a `#[step_args]` block on `foo` it does not collide with.
///
/// Mutates `ctx` in place: it removes the matched placeholder from
/// `ctx.placeholders` and appends the classified [`Arg`] to `ctx.extracted`.
///
/// # Errors
///
/// Returns an error when the lookup name collides with a `#[step_args]` blocked
/// placeholder (via [`validate_no_step_struct_conflict`]) or when a normalized
/// implicit fixture name is not a valid identifier (via
/// [`resolve_fixture_name`]).
fn classify_by_placeholder_match(
    ctx: &mut ClassificationContext,
    from_name: Option<syn::Ident>,
    pat: syn::Ident,
    ty: syn::Type,
) -> syn::Result<()> {
    let pat_name = pat.to_string();
    // Implicit name (the fallback) is normalized; an explicit `#[from(...)]`
    // identifier is taken verbatim.
    let lookup = from_name.as_ref().map_or_else(
        || normalize_param_name(&pat_name).to_owned(),
        ToString::to_string,
    );
    // The `#[step_args]` conflict guard reads only `ctx.extracted`, so run it
    // once before the branch. Keeping it out of both arms stops the two paths
    // from diverging — the shape that previously let `_foo` skip the guard.
    validate_no_step_struct_conflict(ctx, &lookup, &pat)?;
    if ctx.placeholders.remove(&lookup) {
        ctx.extracted.push(Arg::Step { pat, ty });
    } else {
        let name = resolve_fixture_name(from_name, &pat, &lookup)?;
        ctx.extracted.push(Arg::Fixture { pat, name, ty });
    }
    Ok(())
}

/// Classify `arg` as a step argument (placeholder match) or fixture.
///
/// Consumes any `#[from(...)]` attribute on `arg` to determine the lookup name,
/// then claims a matching placeholder from `ctx.placeholders` or falls back to
/// fixture injection. Always returns `Ok(true)`: this is the terminal classifier
/// in the pipeline, so a parameter reaching it is claimed either as a step
/// argument or as a fixture, and `Ok(false)` is never returned. A caller must
/// therefore treat an error as the only non-claiming outcome.
///
/// # Mutation contract
///
/// Mutates both `arg` and `ctx`:
///
/// - Any `#[from]` attribute is removed from [`syn::PatType::attrs`] in place via
///   [`parse_from_attribute`], including on the error paths.
/// - A matched placeholder is removed from `ctx.placeholders`, so it cannot bind a second
///   parameter.
/// - The resulting [`Arg::Step`] or [`Arg::Fixture`] is appended to `ctx.extracted`.
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

#[cfg(test)]
mod tests {
    //! Regressions for the explicit-`#[from(...)]` exactness contract.
    //!
    //! The classifier's broader unit tests live in `classify::tests`; these two
    //! stay beside the code because they pin the distinction that is easiest to
    //! collapse by accident — that an explicit name is matched and guarded
    //! verbatim, while only an implicit one is normalized.

    use std::collections::HashSet;

    use proc_macro2::Span;
    use syn::parse_quote;

    use super::{super::ExtractedArgs, *};

    fn ident(name: &str) -> syn::Ident { syn::Ident::new(name, Span::call_site()) }

    /// Classify `#[from(<from_name>)] state: usize` against the given state.
    fn classify_explicit_from(
        extracted: &mut ExtractedArgs,
        placeholders: &mut HashSet<String>,
        from_name: &str,
    ) -> syn::Result<bool> {
        let mut arg: syn::PatType = parse_quote!(state: usize);
        let from_ident = ident(from_name);
        arg.attrs.push(parse_quote!(#[from(#from_ident)]));
        let ty: syn::Type = parse_quote!(usize);
        let mut ctx = ClassificationContext::new(extracted, placeholders);
        classify_fixture_or_step(&mut ctx, &mut arg, ident("state"), ty)
    }

    #[test]
    fn explicit_from_name_does_not_bind_its_normalized_placeholder() {
        let mut extracted = ExtractedArgs::default();
        let mut placeholders = HashSet::from(["foo".to_owned()]);

        let Ok(handled) = classify_explicit_from(&mut extracted, &mut placeholders, "_foo") else {
            panic!("classification should succeed");
        };

        // `#[from(_foo)]` requests the literal `_foo` key. The placeholder `foo`
        // is a different name, so it must survive unconsumed and the parameter
        // must keep the explicit fixture name rather than becoming a step arg.
        assert!(handled);
        assert!(
            placeholders.contains("foo"),
            "placeholder `foo` should not be consumed by #[from(_foo)]"
        );
        assert!(matches!(
            extracted.args.as_slice(),
            [Arg::Fixture { name, .. }] if name == &ident("_foo")
        ));
    }

    #[test]
    fn explicit_from_name_is_guarded_verbatim_against_blocked_placeholders() {
        let mut extracted = ExtractedArgs::default();
        let idx = extracted.push(Arg::StepStruct {
            pat: ident("args"),
            ty: parse_quote!(Args),
        });
        extracted.step_struct_idx = Some(idx);
        extracted.blocked_placeholders.insert("blocked".to_owned());
        let mut placeholders = HashSet::new();

        let result = classify_explicit_from(&mut extracted, &mut placeholders, "_blocked");

        // The struct owns `blocked`, not `_blocked`, so an explicit request for
        // the literal `_blocked` key does not collide with it. The implicit
        // spelling still does, which
        // `classify_fixture_or_step_respects_blocked_placeholders` pins.
        assert!(
            result.is_ok(),
            "#[from(_blocked)] should not collide with a block on `blocked`"
        );
        assert!(matches!(
            extracted.args.as_slice(),
            [Arg::StepStruct { .. }, Arg::Fixture { name, .. }] if name == &ident("_blocked")
        ));
    }
}
