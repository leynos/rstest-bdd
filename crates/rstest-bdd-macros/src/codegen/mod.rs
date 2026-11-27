//! Code-generation utilities that resolve fully qualified runtime-crate paths,
//! including renamed `rstest-bdd` dependencies, without depending on runtime.
use proc_macro_crate::{FoundCrate, crate_name};
use proc_macro2::{Ident, Span, TokenStream as TokenStream2};
use quote::quote;
use rstest_bdd_policy::TestAttributeHint;
/// Package and crate-name metadata used while resolving generated paths.
struct CrateSpec {
    /// Cargo package name supplied to `proc_macro_crate`.
    package_name: &'static str,
    /// Default Rust crate identifier when the package is not renamed.
    default_crate_name: &'static str,
    /// Adapter type names that establish first-party adapter evidence.
    adapter_type_names: &'static [&'static str],
}
mod policy_diagnostics;
pub(crate) mod scenario;
pub(crate) mod tracking;
pub(crate) mod wrapper;
#[cfg(test)]
pub(crate) use policy_diagnostics::Framework;
pub(crate) use policy_diagnostics::{
    CrateResolver,
    ScenarioFacts,
    SharedPolicyDiagnostics,
    SlotClass,
    classify_harness_path,
    classify_policy_path,
    default_resolver,
    selected_hint,
};
/// Specification for the core `rstest-bdd` runtime crate.
const RSTEST_BDD: CrateSpec = CrateSpec {
    package_name: "rstest-bdd",
    default_crate_name: "rstest_bdd",
    adapter_type_names: &[],
};
/// Specification for the base `rstest-bdd-harness` crate.
const RSTEST_BDD_HARNESS: CrateSpec = CrateSpec {
    package_name: "rstest-bdd-harness",
    default_crate_name: "rstest_bdd_harness",
    adapter_type_names: &[],
};
/// Specification for the first-party Tokio harness adapter.
const TOKIO_HARNESS: CrateSpec = CrateSpec {
    package_name: "rstest-bdd-harness-tokio",
    default_crate_name: "rstest_bdd_harness_tokio",
    adapter_type_names: &["TokioHarness", "TokioAttributePolicy"],
};
/// Specification for the first-party GPUI harness adapter.
const GPUI_HARNESS: CrateSpec = CrateSpec {
    package_name: "rstest-bdd-harness-gpui",
    default_crate_name: "rstest_bdd_harness_gpui",
    adapter_type_names: &["GpuiHarness", "GpuiAttributePolicy"],
};
/// Return a token stream pointing to the `rstest_bdd` crate or its renamed form.
#[cfg(not(test))]
pub(crate) fn rstest_bdd_path() -> TokenStream2 { resolve_crate_path(&RSTEST_BDD) }
#[cfg(test)]
pub(crate) fn rstest_bdd_path() -> TokenStream2 { quote!(::rstest_bdd) }
/// Return a token stream pointing to the `rstest_bdd_harness` crate or its
/// renamed form.
pub(crate) fn rstest_bdd_harness_path() -> TokenStream2 { resolve_crate_path(&RSTEST_BDD_HARNESS) }
/// Return a token stream pointing to the `rstest_bdd_harness_tokio` crate or
/// its renamed form.
///
/// Used by the `runtime = "tokio-current-thread"` compatibility alias to
/// resolve `TokioHarness` via proper crate lookup, supporting downstream
/// crates that rename the dependency in their `Cargo.toml`.
pub(crate) fn rstest_bdd_harness_tokio_path() -> TokenStream2 { resolve_crate_path(&TOKIO_HARNESS) }
/// Return the crate root that provides base harness API for the given harness
/// or attribute-policy path.
///
/// Adapter detection relies on the crate-root identifier in `adapter_path`
/// matching either the default snake-case crate name or a resolved renamed
/// dependency. Renamed dependencies that cannot be resolved at
/// macro-expansion time (including test builds) fall back to the base
/// `rstest-bdd-harness` path.
#[derive(Clone)]
pub(crate) struct HarnessApiResolution {
    /// Base harness API crate path selected for generated code.
    pub(crate) api_path: TokenStream2,
}

/// Resolve the base harness API path using an injected crate resolver.
pub(crate) fn resolve_harness_api_with(
    adapter_path: &syn::Path,
    resolver: &dyn CrateResolver,
) -> HarnessApiResolution {
    HarnessApiResolution {
        api_path: first_party_adapter_api_path(adapter_path, resolver)
            .unwrap_or_else(rstest_bdd_harness_path),
    }
}

/// Return the generated-test attribute hint for a recognized first-party adapter.
pub(crate) fn first_party_adapter_attribute_hint(
    adapter_path: &syn::Path,
    resolver: &dyn CrateResolver,
) -> Option<TestAttributeHint> {
    if first_party_adapter_path_matches(adapter_path, &TOKIO_HARNESS, resolver) {
        Some(TestAttributeHint::RstestWithTokioCurrentThread)
    } else if first_party_adapter_path_matches(adapter_path, &GPUI_HARNESS, resolver) {
        Some(TestAttributeHint::RstestWithGpuiTest)
    } else {
        None
    }
}

/// Resolve a recognized first-party adapter path to its API crate root.
fn first_party_adapter_api_path(
    adapter_path: &syn::Path,
    resolver: &dyn CrateResolver,
) -> Option<TokenStream2> {
    first_party_adapter_spec(adapter_path, resolver)
        .map(|spec| first_party_adapter_api_root(adapter_path, spec, resolver))
}

/// Return the first-party adapter specification matching a supplied path.
fn first_party_adapter_spec(
    adapter_path: &syn::Path,
    resolver: &dyn CrateResolver,
) -> Option<&'static CrateSpec> {
    [&TOKIO_HARNESS, &GPUI_HARNESS]
        .into_iter()
        .find(|spec| first_party_adapter_path_matches(adapter_path, spec, resolver))
}
/// Select the API root for a recognized adapter path.
fn first_party_adapter_api_root(
    adapter_path: &syn::Path,
    spec: &CrateSpec,
    resolver: &dyn CrateResolver,
) -> TokenStream2 {
    if path_root_matches_crate(adapter_path, spec, resolver) {
        let Some(root) = adapter_path.segments.first().map(|segment| &segment.ident) else {
            return resolve_crate_path(spec);
        };
        quote! { ::#root }
    } else {
        resolve_crate_path(spec)
    }
}

/// Determine whether a path has sufficient evidence for a first-party adapter.
fn first_party_adapter_path_matches(
    adapter_path: &syn::Path,
    spec: &CrateSpec,
    resolver: &dyn CrateResolver,
) -> bool {
    path_last_ident_matches(adapter_path, spec.adapter_type_names)
        && (path_root_matches_crate(adapter_path, spec, resolver)
            || is_imported_adapter_type_path(adapter_path, spec, resolver))
}

/// Determine whether an unqualified imported adapter type can be resolved.
///
/// Recognition asks Cargo for a root, not merely for a dependency: a bare
/// `TokioHarness` is only first-party when the adapter crate is visible to the
/// generated code. Evidence asks the weaker question separately, which is why
/// the two resolver methods exist.
fn is_imported_adapter_type_path(
    path: &syn::Path,
    spec: &CrateSpec,
    resolver: &dyn CrateResolver,
) -> bool {
    path.segments.len() == 1 && resolver.resolved_root(spec.package_name).is_some()
}

/// Determine whether the final path segment is one of the expected identifiers.
fn path_last_ident_matches(path: &syn::Path, expected: &[&str]) -> bool {
    path.segments
        .last()
        .is_some_and(|segment| expected.iter().any(|name| segment.ident == name))
}

/// Determine whether the root segment names a first-party adapter crate.
fn path_root_matches_crate(
    path: &syn::Path,
    spec: &CrateSpec,
    resolver: &dyn CrateResolver,
) -> bool {
    let Some(root) = path.segments.first() else {
        return false;
    };
    root.ident == spec.default_crate_name
        || resolver
            .resolved_root(spec.package_name)
            .is_some_and(|resolved| resolved == root.ident)
}

/// Resolve a dependency specification to a generated crate path.
fn resolve_crate_path(spec: &CrateSpec) -> TokenStream2 {
    match crate_name(spec.package_name) {
        Ok(found) => found_crate_path(found, spec),
        Err(err) => handle_missing_crate(spec, &err),
    }
}

/// Convert a resolved dependency into a fully-qualified token path.
fn found_crate_path(found: FoundCrate, spec: &CrateSpec) -> TokenStream2 {
    let ident = match found {
        FoundCrate::Itself => Ident::new(spec.default_crate_name, Span::call_site()),
        FoundCrate::Name(name) => Ident::new(&name, Span::call_site()),
    };
    quote! { ::#ident }
}

#[cfg(test)]
/// Provide a deterministic default crate path for isolated macro tests.
fn handle_missing_crate(spec: &CrateSpec, _: &proc_macro_crate::Error) -> TokenStream2 {
    // Tests compile the macros crate in isolation without dependency crates, so
    // fall back to the default package name.
    let ident = Ident::new(spec.default_crate_name, Span::call_site());
    quote! { ::#ident }
}

#[cfg(not(test))]
/// Abort expansion when a required generated-code dependency is unavailable.
fn handle_missing_crate(spec: &CrateSpec, err: &proc_macro_crate::Error) -> TokenStream2 {
    let crate_name = spec.package_name;
    panic!("{crate_name} crate not found: {err}");
}

#[cfg(test)]
#[path = "tests/mod.rs"]
mod tests;
