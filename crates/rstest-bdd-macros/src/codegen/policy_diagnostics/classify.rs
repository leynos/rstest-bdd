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

/// A Cargo-state read that could not be answered.
///
/// `CrateNotFound` is not an error: it is the normal answer for a package the
/// caller does not depend on. A manifest that cannot be read or parsed is
/// different, and folding it into absence would silently downgrade recognition
/// and suggested paths, so it stays explicit.
#[derive(Debug)]
pub(crate) struct ResolveError {
    /// The package whose Cargo state could not be read.
    pub(crate) package: String,
    /// The underlying failure.
    pub(crate) source: proc_macro_crate::Error,
}

impl std::fmt::Display for ResolveError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "could not read the Cargo state for `{}`: {}",
            self.package, self.source
        )
    }
}

/// Cargo's answer about one first-party adapter crate.
pub(crate) trait CrateResolver {
    /// The root identifier under which `package_name` is a direct dependency.
    ///
    /// `Ok(None)` is the normal "not a dependency" answer; `Err` means the
    /// manifest state could not be read at all.
    fn resolved_root(&self, package_name: &str) -> Result<Option<Ident>, ResolveError>;

    /// Whether syntactic evidence alone may treat `package_name` as present.
    ///
    /// Production ties this to [`Self::resolved_root`]; the unit-test
    /// implementation keeps the historical stub in which a syntactic
    /// candidate counts as present even though no renamed root can match.
    fn is_direct_dependency(&self, package_name: &str) -> Result<bool, ResolveError>;
}

/// The resolver used by every real macro expansion.
#[cfg(not(test))]
pub(crate) struct ProcMacroCrateResolver;

#[cfg(not(test))]
impl CrateResolver for ProcMacroCrateResolver {
    fn resolved_root(&self, package_name: &str) -> Result<Option<Ident>, ResolveError> {
        match proc_macro_crate::crate_name(package_name) {
            Ok(found) => {
                let name = match found {
                    proc_macro_crate::FoundCrate::Itself => package_name.replace('-', "_"),
                    proc_macro_crate::FoundCrate::Name(name) => name,
                };
                Ok(Some(Ident::new(&name, proc_macro2::Span::call_site())))
            }
            Err(proc_macro_crate::Error::CrateNotFound { .. }) => Ok(None),
            Err(source) => Err(ResolveError {
                package: package_name.to_owned(),
                source,
            }),
        }
    }

    fn is_direct_dependency(&self, package_name: &str) -> Result<bool, ResolveError> {
        self.resolved_root(package_name).map(|root| root.is_some())
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
    fn resolved_root(&self, _: &str) -> Result<Option<Ident>, ResolveError> { Ok(None) }

    fn is_direct_dependency(&self, _: &str) -> Result<bool, ResolveError> { Ok(true) }
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
/// Evidence needs three things together: the adapter type name in the final
/// position, the crate's identifier somewhere below the root, and the crate
/// resolvable as a direct dependency. A path that merely ends in
/// `TokioHarness` is a custom type (ADR-0017), and a canonical root is
/// recognition rather than evidence, so it never reaches this function.
fn evidence_framework(path: &syn::Path, resolver: &dyn CrateResolver) -> Option<Framework> {
    [Framework::Tokio, Framework::Gpui]
        .into_iter()
        .find(|framework| {
            let spec = adapter_spec(*framework);
            path_last_ident_matches(path, spec.adapter_type_names)
                && crate_identifier_below_root(path, spec, resolver)
                && resolves_to(resolver.is_direct_dependency(spec.package_name))
        })
}

/// Whether the adapter crate's identifier appears anywhere below the root.
///
/// Both spellings count: the canonical crate identifier, and — for a renamed
/// dependency — the root identifier Cargo resolved it to, so a nested path
/// such as `a::tok::TokioHarness` still carries evidence.
fn crate_identifier_below_root(
    path: &syn::Path,
    spec: &CrateSpec,
    resolver: &dyn CrateResolver,
) -> bool {
    let renamed = read_root(resolver.resolved_root(spec.package_name));
    path.segments.iter().skip(1).any(|segment| {
        segment.ident == spec.default_crate_name
            || renamed.as_ref().is_some_and(|root| segment.ident == *root)
    })
}

/// The crate specification for a framework.
pub(super) fn adapter_spec(framework: Framework) -> &'static CrateSpec {
    match framework {
        Framework::Tokio => &TOKIO_HARNESS,
        Framework::Gpui => &GPUI_HARNESS,
    }
}

/// A boolean Cargo answer, with an unreadable manifest treated as absence.
///
/// Absence is the conservative reading: it never claims first-party evidence
/// the manifest did not establish. [`SharedPolicyDiagnostics`](super::SharedPolicyDiagnostics)
/// reports the unreadable manifest itself, so the absence is not silent.
fn resolves_to(answer: Result<bool, ResolveError>) -> bool { answer.unwrap_or(false) }

/// A resolved root, with an unreadable manifest treated as unresolvable.
fn read_root(answer: Result<Option<Ident>, ResolveError>) -> Option<Ident> {
    answer.unwrap_or_default()
}
