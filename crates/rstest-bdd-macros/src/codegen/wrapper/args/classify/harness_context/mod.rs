//! Classifier for the `#[harness_context]` step-parameter marker.
//!
//! A parameter annotated `#[harness_context]` requests the harness-provided
//! context object stored under the reserved `rstest_bdd_harness_context`
//! fixture key. The classifier converts the readable marker into the same
//! [`Arg::Fixture`] the legacy `#[from(rstest_bdd_harness_context)]` spelling
//! produces, so generated wrappers are byte-identical for both spellings.

use std::collections::HashSet;

use quote::ToTokens;

use super::{Arg, ExtractedArgs, extract_flag_attribute};

#[cfg(test)]
mod prop_tests;
#[cfg(test)]
mod tests;

/// Diagnostic for `#[harness_context]` combined with `#[from]`.
pub(super) const HARNESS_CONTEXT_WITH_FROM_ERROR: &str =
    "`#[harness_context]` cannot be combined with `#[from]`";

/// Diagnostic for `#[harness_context]` combined with `#[datatable]`.
pub(super) const HARNESS_CONTEXT_WITH_DATATABLE_ERROR: &str =
    "`#[harness_context]` cannot be combined with `#[datatable]`";

/// Diagnostic for `#[harness_context]` combined with `#[step_args]`.
///
/// Shared with the `#[step_args]` cross-guard in `classify::step_struct`, so
/// users see one story regardless of attribute order.
pub(super) const HARNESS_CONTEXT_WITH_STEP_ARGS_ERROR: &str =
    "`#[harness_context]` cannot be combined with `#[step_args]`";

/// Reject every combination that would otherwise let the marker leak into
/// generated code.
///
/// The three attribute pairs are checked from a table; the placeholder check
/// is a single predicate, keeping this helper within the branch limits the
/// Whitaker suite enforces.
///
/// # Errors
///
/// Returns an error spanned over `arg` when the parameter also carries
/// `#[from]`, `#[datatable]`, or `#[step_args]`, or when the parameter's name
/// is bound to a step-pattern placeholder.
fn validate_harness_context_marker(
    arg: &syn::PatType,
    pat: &syn::Ident,
    placeholders: &HashSet<String>,
) -> syn::Result<()> {
    for (attr_name, message) in [
        ("from", HARNESS_CONTEXT_WITH_FROM_ERROR),
        ("datatable", HARNESS_CONTEXT_WITH_DATATABLE_ERROR),
        ("step_args", HARNESS_CONTEXT_WITH_STEP_ARGS_ERROR),
    ] {
        if arg.attrs.iter().any(|a| a.path().is_ident(attr_name)) {
            return Err(syn::Error::new_spanned(arg, message));
        }
    }
    if placeholders.contains(&pat.to_string()) {
        return Err(syn::Error::new_spanned(
            arg,
            format!("`#[harness_context]` cannot bind step-argument placeholder `{pat}`"),
        ));
    }
    Ok(())
}

/// Classify `arg` as the harness-context fixture, if the marker is present.
///
/// The classifier runs first in the extraction pipeline, before the
/// placeholder short-circuit, so the marker can never leak into generated
/// code. When the marker is absent it returns `Ok(false)` and leaves the
/// parameter for the next classifier.
pub(crate) fn classify_harness_context(
    st: &mut ExtractedArgs,
    arg: &mut syn::PatType,
    placeholders: &HashSet<String>,
) -> syn::Result<bool> {
    let via_marker = extract_flag_attribute(arg, "harness_context")?;
    if !via_marker {
        return Ok(false);
    }

    let pat = match &*arg.pat {
        syn::Pat::Ident(pat_ident) => pat_ident.ident.clone(),
        other => {
            let pattern = other.to_token_stream().to_string();
            return Err(syn::Error::new(
                proc_macro2::Span::call_site(),
                format!(
                    "unsupported parameter pattern `{pattern}`; use a simple identifier (e.g., \
                     `arg: T`)"
                ),
            ));
        }
    };
    let ty = (*arg.ty).clone();

    validate_harness_context_marker(arg, &pat, placeholders)?;

    // `HARNESS_CONTEXT_FIXTURE` is a compile-time constant that is a valid Rust
    // identifier; `classify::harness_context::tests` pins that invariant, so
    // `Ident::new` cannot panic here.
    let name = syn::Ident::new(rstest_bdd_policy::HARNESS_CONTEXT_FIXTURE, pat.span());
    st.push(Arg::Fixture {
        pat: pat.clone(),
        name,
        ty: ty.clone(),
    });
    Ok(true)
}
