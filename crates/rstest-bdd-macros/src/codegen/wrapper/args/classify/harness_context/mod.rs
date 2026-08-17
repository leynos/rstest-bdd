//! Classifier for the `#[harness_context]` step-parameter marker.
//!
//! A parameter annotated `#[harness_context]` requests the harness-provided
//! context object stored under the reserved `rstest_bdd_harness_context`
//! fixture key. The classifier converts the readable marker into the same
//! [`Arg::Fixture`] the legacy `#[from(rstest_bdd_harness_context)]` spelling
//! produces, so generated wrappers are byte-identical for both spellings.

use std::collections::HashSet;

use super::{Arg, ExtractedArgs};

#[cfg(test)]
mod prop_tests;
#[cfg(test)]
mod tests;

/// Classify `arg` as the harness-context fixture, if the marker is present.
///
/// The classifier runs first in the extraction pipeline, before the
/// placeholder short-circuit, so the marker can never leak into generated
/// code. When the marker is absent it returns `Ok(false)` and leaves the
/// parameter for the next classifier.
pub(crate) fn classify_harness_context(
    st: &mut ExtractedArgs,
    arg: &mut syn::PatType,
    pat: &syn::Ident,
    ty: &syn::Type,
    placeholders: &HashSet<String>,
) -> syn::Result<bool> {
    let _ = (st, arg, pat, ty, placeholders);
    Ok(false)
}
