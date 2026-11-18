//! The ownership list both families declare, exercised against the real list.
//!
//! [`super::super::WORKSPACE_DEPENDENCIES`] is a *declaration*, and the
//! harness's ownership tests can only prove the mechanism works for whichever
//! list a case supplies. Neither half on its own catches a list that has
//! drifted from the fixtures' actual dependency graph: the mechanism stays
//! correct while the declaration quietly omits a crate the build compiles.
//! This case drives the list the families actually use, so a missing entry
//! fails here rather than as a rebuild inside a timed assertion.
//!
//! The crates named are mostly *transitive*. Each fixture's manifest names
//! only `rstest-bdd` and `rstest-bdd-macros`; `rstest-bdd` pulls in
//! `rstest-bdd-patterns` and `rstest-bdd-policy`, and `rstest-bdd-macros`
//! pulls in those two plus `rstest-bdd-harness`. A list built from the
//! manifests alone covers the direct edges and leaves the rest unprotected,
//! which is the defect this case exists to catch one level down.

use super::super::{ADDITION, REBUILD, WORKSPACE_DEPENDENCIES, fixture_preparation};
use crate::harness::process;

/// The workspace crates one scratch fixture builds, read from its own graph.
///
/// `cargo metadata` is what decides, not a compile: it resolves the fixture's
/// own manifest and lockfile and reports every package without building one.
/// Only path packages are returned, because those are the ones a preparation
/// compiles from this checkout; a registry crate is immutable and already
/// pinned by the fixture's lockfile, so its units are neither declared nor at
/// risk of a local edit.
///
/// # Errors
///
/// Returns the resolution error when the manifest cannot be read, and names
/// the manifest or the package when the graph lacks what the walk needs — an
/// unwritten `resolve` section, or the fixture absent from its own manifest.
/// Each of those is a different broken assumption, and a caller told "the
/// graph did not resolve" would look in the wrong place for the first two.
fn local_dependency_closure(
    manifest: &std::path::Path,
    package: &str,
) -> Result<std::collections::BTreeSet<String>, Box<dyn std::error::Error>> {
    let metadata = cargo_metadata::MetadataCommand::new()
        .manifest_path(manifest)
        // The fixture commits its lockfile, so resolution needs no registry:
        // asserting that keeps a test of the declaration from reaching the
        // network, where a registry hiccup would read as a broken graph.
        .other_options(vec!["--locked".to_owned(), "--offline".to_owned()])
        .exec()?;
    let resolve = metadata
        .resolve
        .as_ref()
        .ok_or("the fixture manifest resolved to no dependency graph")?;
    let packages: std::collections::BTreeMap<_, _> = metadata
        .packages
        .iter()
        .map(|package| (&package.id, package))
        .collect();

    // Walk the resolved graph from the fixture. `deps` names the package ids
    // each node resolved to, so following them reaches the whole closure
    // without the fixture's sources being compiled.
    let fixture = metadata
        .packages
        .iter()
        .find(|candidate| candidate.name.as_str() == package)
        .ok_or_else(|| format!("{package} is not in the manifest at {}", manifest.display()))?;
    let nodes: std::collections::BTreeMap<_, _> =
        resolve.nodes.iter().map(|node| (&node.id, node)).collect();
    let mut reached = std::collections::BTreeSet::new();
    let mut pending = vec![&fixture.id];
    while let Some(id) = pending.pop() {
        if !reached.insert(id) {
            continue;
        }
        if let Some(node) = nodes.get(id) {
            pending.extend(node.deps.iter().map(|dep| &dep.pkg));
        }
    }

    Ok(reached
        .into_iter()
        .filter_map(|id| packages.get(id))
        .filter(|candidate| candidate.source.is_none() && candidate.name.as_str() != package)
        .map(|candidate| candidate.name.to_string())
        .collect())
}

/// The declaration agrees with what each fixture's real graph resolves to.
///
/// [`WORKSPACE_DEPENDENCIES`] is a handwritten copy of the crate names a
/// preparation builds, and the ownership mechanism cannot catch it going
/// stale: a crate the fixture gains is missing from the declaration *and* from
/// the recorded name list, so it compares equal to itself and the gate passes
/// while the timed rebuild compiles it. This case reads each fixture's graph
/// instead of restating it, so the declaration fails only when the tree and
/// the constant actually disagree.
///
/// It also drives the preparation the family really uses rather than a copy of
/// it. Asserting the constant alone would leave the wiring untested: a
/// `fixture_preparation` that assigned an empty list would leave the constant
/// intact and every other case in the preparation tests green, because they all
/// stamp a profile with the closure they record filtered down to. Both halves
/// must be checked, and the declaration is checked against the graph rather than
/// against itself.
///
/// The fixture package is deliberately *not* in the declaration, and so is not
/// in the expected set below. Each experiment runs `cargo clean -p <fixture>`
/// before its baseline by design, which removes that package's `.d` records and
/// its build-script `out/` directories. A stamp that recorded them would be
/// refused by the very next assertion, because the experiment destroys them on
/// purpose: rebuilding those units is the contract under test, so they are
/// never "prepared". Claiming them would make the preparation defeat itself.
#[test]
fn the_declaration_matches_the_fixtures_resolved_graph() {
    let workspace = super::super::workspace_root();
    for (family, manifest) in [
        (
            REBUILD,
            workspace.join("crates/rstest-bdd/tests/fixtures/rebuild_invalidation/Cargo.toml"),
        ),
        (
            ADDITION,
            workspace.join("crates/rstest-bdd/tests/fixtures/feature_addition/Cargo.toml"),
        ),
    ] {
        let local = local_dependency_closure(&manifest, family.package)
            .expect("the fixture's dependency graph must resolve");
        let declared: std::collections::BTreeSet<String> = WORKSPACE_DEPENDENCIES
            .iter()
            .map(|name| (*name).to_owned())
            .collect();
        assert_eq!(
            local, declared,
            "WORKSPACE_DEPENDENCIES must be exactly what {} resolves to; the timed suite rebuilds \
             any crate it omits",
            family.name
        );

        // The constant is only half the claim: the family has to hand it to
        // the preparation as well, or the gate filters the dependency's records
        // out of both sides and cannot see the eviction at all.
        let environment = process::child_env_from_vars(Vec::new())
            .expect("the fixture environment should resolve");
        let preparation =
            fixture_preparation(&environment, family).expect("preparation description");
        let owned: std::collections::BTreeSet<&str> = preparation
            .owned_packages
            .iter()
            .map(String::as_str)
            .collect();
        let expected: std::collections::BTreeSet<&str> =
            WORKSPACE_DEPENDENCIES.iter().copied().collect();
        assert_eq!(
            owned, expected,
            "{} must declare every dependency crate it compiles, and only those: a dependency \
             left out of `owned_packages` is filtered out of both the recorded and the present \
             lists and an eviction of it passes the gate, while the fixture itself is removed by \
             the experiment's own `cargo clean -p` before every baseline",
            family.name
        );
    }
}
