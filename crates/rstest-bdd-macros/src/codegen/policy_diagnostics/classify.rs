//! Classify a supplied `syn::Path` into a [`SlotClass`].
//!
//! Recognition must agree with code generation exactly: a path is
//! `Recognized` when — and only when — the attribute-policy resolver and the
//! first-party adapter hint agree that it names a first-party adapter. A path
//! that is not recognized but still carries first-party evidence is
//! `Evidenced`; everything else is `Foreign` and stays silent (ADR-0008 keeps
//! third-party harnesses valid without an attribute policy).
//!
//! Cargo state is read through an injected [`CrateResolver`] so unit tests can
//! reach the renamed-dependency and unresolvable branches without a real
//! manifest. Production uses [`ProcMacroCrateResolver`]; unit tests use
//! [`SyntacticCrateResolver`], which keeps the historical fiction that a
//! syntactic candidate is resolvable.

use proc_macro2::Ident;
use rstest_bdd_policy::{
    TestAttributeHint,
    resolve_test_attribute_hint_for_harness_path,
    resolve_test_attribute_hint_for_policy_path,
};

use super::model::{Framework, SlotClass};
use crate::codegen::{
    CrateSpec,
    GPUI_HARNESS,
    TOKIO_HARNESS,
    first_party_adapter_attribute_hint,
    path_last_ident_matches,
};

/// Cargo's answer about one first-party adapter crate.
pub(crate) trait CrateResolver {
    /// The root identifier under which `package_name` is a direct dependency,
    /// or `None` when it is not one.
    fn resolved_root(&self, package_name: &str) -> Option<Ident>;

    /// Whether syntactic evidence alone may treat `package_name` as present.
    ///
    /// Production ties this to [`Self::resolved_root`]; the unit-test
    /// implementation keeps the historical stub in which a syntactic
    /// candidate counts as present even though no renamed root can match.
    fn is_direct_dependency(&self, package_name: &str) -> bool;
}

/// The resolver used by every real macro expansion.
#[cfg(not(test))]
pub(crate) struct ProcMacroCrateResolver;

#[cfg(not(test))]
impl CrateResolver for ProcMacroCrateResolver {
    fn resolved_root(&self, package_name: &str) -> Option<Ident> {
        let found = proc_macro_crate::crate_name(package_name).ok()?;
        let name = match found {
            proc_macro_crate::FoundCrate::Itself => package_name.replace('-', "_"),
            proc_macro_crate::FoundCrate::Name(name) => name,
        };
        Some(Ident::new(&name, proc_macro2::Span::call_site()))
    }

    fn is_direct_dependency(&self, package_name: &str) -> bool {
        proc_macro_crate::crate_name(package_name).is_ok()
    }
}

/// The resolver unit tests see by default.
///
/// The macros crate does not depend on the adapter crates, so a real resolver
/// reports them absent in a unit test. Reusing that answer would make every
/// syntactic case unresolvable and change the meaning of the existing tests;
/// this stub keeps the pre-existing split, where a syntactic candidate carries
/// evidence but no renamed root matches.
#[cfg(test)]
pub(crate) struct SyntacticCrateResolver;

#[cfg(test)]
impl CrateResolver for SyntacticCrateResolver {
    fn resolved_root(&self, _: &str) -> Option<Ident> { None }

    fn is_direct_dependency(&self, _: &str) -> bool { true }
}

/// The resolver a boundary uses when none is injected.
#[cfg(not(test))]
pub(crate) fn default_resolver() -> &'static dyn CrateResolver { &ProcMacroCrateResolver }

/// The resolver a boundary uses when none is injected.
#[cfg(test)]
pub(crate) fn default_resolver() -> &'static dyn CrateResolver { &SyntacticCrateResolver }

/// Classify a `harness = ...` path.
pub(crate) fn classify_harness_path(path: &syn::Path, resolver: &dyn CrateResolver) -> SlotClass {
    classify(path, resolver, resolve_test_attribute_hint_for_harness_path)
}

/// Classify an `attributes = ...` path.
pub(crate) fn classify_policy_path(path: &syn::Path, resolver: &dyn CrateResolver) -> SlotClass {
    classify(path, resolver, resolve_test_attribute_hint_for_policy_path)
}

/// A policy-table lookup for one slot kind.
type Lookup = fn(&[&str]) -> Option<TestAttributeHint>;

/// Classify through the slot's own policy table and the shared adapter hint.
fn classify(path: &syn::Path, resolver: &dyn CrateResolver, lookup: Lookup) -> SlotClass {
    if let Some(hint) = recognized_hint(path, resolver, lookup) {
        return SlotClass::Recognized(hint);
    }
    evidence_framework(path, resolver).map_or(SlotClass::Foreign, SlotClass::Evidenced)
}

/// The hint a path selects, using exactly the code generator's predicates.
fn recognized_hint(
    path: &syn::Path,
    resolver: &dyn CrateResolver,
    lookup: Lookup,
) -> Option<TestAttributeHint> {
    let names = segment_names(path);
    let segments = names.iter().map(String::as_str).collect::<Vec<_>>();
    lookup(&segments).or_else(|| first_party_adapter_attribute_hint(path, resolver))
}

/// The path's segments, as the policy tables spell them.
fn segment_names(path: &syn::Path) -> Vec<String> {
    path.segments
        .iter()
        .map(|segment| segment.ident.to_string())
        .collect::<Vec<_>>()
}

/// The framework a path evidences, when it evidences one.
///
/// Today's rule is deliberately narrow: the adapter crate identifier must be
/// the segment immediately before the adapter type, and the crate must be a
/// direct dependency. A path that merely ends in `TokioHarness` is a custom
/// type (ADR-0017).
fn evidence_framework(path: &syn::Path, resolver: &dyn CrateResolver) -> Option<Framework> {
    [Framework::Tokio, Framework::Gpui]
        .into_iter()
        .find(|framework| {
            let spec = adapter_spec(*framework);
            path_last_ident_matches(path, spec.adapter_type_names)
                && penultimate_ident_matches(path, spec.default_crate_name)
                && resolver.is_direct_dependency(spec.package_name)
        })
}

/// The crate specification for a framework.
pub(super) fn adapter_spec(framework: Framework) -> &'static CrateSpec {
    match framework {
        Framework::Tokio => &TOKIO_HARNESS,
        Framework::Gpui => &GPUI_HARNESS,
    }
}

/// Whether a path's penultimate segment is the expected crate identifier.
fn penultimate_ident_matches(path: &syn::Path, expected: &str) -> bool {
    let mut segments = path.segments.iter().rev();
    let _adapter_type = segments.next();
    segments
        .next()
        .is_some_and(|segment| segment.ident == expected)
}
