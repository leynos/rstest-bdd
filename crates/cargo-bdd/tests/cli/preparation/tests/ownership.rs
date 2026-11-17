//! The ownership list this family declares, exercised against the real list.
//!
//! [`super::fixture_dependency_closure`] is the family's *declaration*: the
//! packages its preparation builds, so the units its stamp may speak for. The
//! harness's own ownership tests can only prove the mechanism works for
//! whichever list a case supplies. Neither half on its own catches a list that
//! has drifted from the fixture's actual dependency graph: the mechanism stays
//! correct while the declaration quietly omits a crate the build compiles. The
//! cases here drive the list the family actually uses, so a missing entry fails
//! here rather than as a rebuild inside a timed assertion.
//!
//! Most of the workspace crates named are *transitive*. Only `rstest-bdd` and
//! `rstest-bdd-macros` appear in the fixture's manifest; `rstest-bdd` pulls in
//! `rstest-bdd-patterns` and `rstest-bdd-policy`, and `rstest-bdd-macros` pulls
//! in those two plus `rstest-bdd-harness`. A list built from the manifest alone
//! covers the direct edges and leaves the rest unprotected, which is the defect
//! these cases exist to catch one level down.

use std::fs;

use super::{FIXTURE_PACKAGE, fixture_dependency_closure, nested_cargo};

/// The family's declared packages, with the resolution error as an `io::Error`.
///
/// [`fixture_dependency_closure`] reports through `eyre`, which the shared
/// preparation fixtures in this module cannot return. The resolution failure
/// is the only error either can produce, so it is carried across as an
/// [`std::io::ErrorKind::Other`] error rather than the callers growing an
/// error type that names nothing further.
fn declared_packages() -> std::io::Result<Vec<String>> {
    fixture_dependency_closure().map_err(|error| std::io::Error::other(error.to_string()))
}

/// A scratch preparation carrying the real family's ownership list.
///
/// The harness's own fixtures live in another crate, so this builds the shape
/// here: one profile below a target root holding compiled units and a
/// fingerprint store, and a stamp root of its own. The ownership list is the
/// family's, not a copy of it — that is the whole point of the case.
///
/// A unit and a record set are written for every declared package, because the
/// preparation now holds the declaration to the record: a package named in
/// `owned_packages` that contributed no artefact is refused. Writing them all
/// keeps the fixture truthful about what it built, so a case's refusal can only
/// come from the eviction it performs.
fn family_preparation(
    scratch: &std::path::Path,
) -> std::io::Result<(nested_cargo::Preparation, std::path::PathBuf)> {
    let target = scratch.join("target");
    let profile = target.join(nested_cargo::DEFAULT_PROFILE);
    let deps = profile.join("deps");
    fs::create_dir_all(&deps)?;
    let store = profile.join(".fingerprint");
    fs::create_dir_all(&store)?;

    let environment = nested_cargo::env_from_vars(
        Vec::new(),
        Some(&target),
        nested_cargo::ProfileDestination::Caller,
    )?;
    let mut preparation = nested_cargo::Preparation::new(
        nested_cargo::Sources::new("cargo-bdd-cli", "test", "hash"),
        scratch.join("stamp-root"),
        environment,
    )?;
    preparation.owned_packages = declared_packages()?;

    // One unit and one record set per package the family claims, named the way
    // Cargo names them: a hyphenated record directory, and an underscored
    // library output. Both carry the same 16-hex-digit metadata hash, which is
    // what pairs an output with the record that keeps it current.
    for package in &preparation.owned_packages {
        let stem = package.replace('-', "_");
        let unit = format!("{stem}-0123456789abcdef");
        fs::write(deps.join(format!("{unit}.d")), b"dep-info")?;
        fs::write(deps.join(format!("lib{unit}.rlib")), b"unit")?;
        let records = store.join(format!("{package}-0123456789abcdef"));
        fs::create_dir_all(&records)?;
        fs::write(records.join("lib-record"), b"record")?;
    }
    Ok((preparation, profile))
}

/// The workspace crates this family's preparation must vouch for, named
/// independently.
///
/// Deliberately *not* read from the declaration. A case that iterated the
/// declaration would inherit every omission it is meant to catch: drop a crate
/// from the list and the loop simply stops checking it, so the case passes
/// whether or not the defect is present.
///
/// Every entry but the first two is transitive. The fixture's manifest names
/// only `rstest-bdd` and `rstest-bdd-macros`; `rstest-bdd` pulls in
/// `rstest-bdd-patterns` and `rstest-bdd-policy`, and `rstest-bdd-macros` pulls
/// in those two plus `rstest-bdd-harness`. All five are compiled by the
/// preparation, so all five have to be declared.
const REQUIRED_DEPENDENCIES: &[&str] = &[
    "rstest-bdd",
    "rstest-bdd-macros",
    "rstest-bdd-harness",
    "rstest-bdd-patterns",
    "rstest-bdd-policy",
];

/// The declaration covers every workspace crate the fixture's manifest reaches.
///
/// `cargo metadata` is what decides, not a compile: it resolves the fixture's
/// own manifest and lockfile and reports every package without building one.
///
/// A *containment* rather than an equality, unlike the harness's version of this
/// case. The declaration is the fixture's whole closure — registry packages
/// included, because their units are compiled into the same profile and a
/// removed fingerprint record rebuilds them all the same — while this case's
/// restated expectation names only the workspace crates. Checking that those
/// five are all present is the claim that would otherwise go stale silently: the
/// mechanism stays correct while a crate the manifest gains disappears from the
/// declaration.
#[test]
fn the_declaration_covers_the_fixtures_workspace_dependencies() {
    let manifest =
        super::super::workspace_root().join("crates/cargo-bdd/tests/fixtures/minimal/Cargo.toml");
    let metadata = cargo_metadata::MetadataCommand::new()
        .manifest_path(manifest)
        // The fixture commits its lockfile, so resolution needs no registry:
        // asserting that keeps a test of the declaration from reaching the
        // network, where a registry hiccup would read as a broken graph.
        .other_options(vec!["--locked".to_owned(), "--offline".to_owned()])
        .exec()
        .expect("the fixture manifest must resolve");
    let resolve = metadata.resolve.as_ref().expect("a resolved graph");
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
        .find(|package| package.name.as_str() == FIXTURE_PACKAGE)
        .expect("the fixture is in its own manifest");
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

    // Only this checkout's packages: a registry crate is immutable, so the
    // restated expectation names the workspace crates the fixture reaches.
    let local: std::collections::BTreeSet<String> = reached
        .into_iter()
        .filter_map(|id| packages.get(id))
        .filter(|package| package.source.is_none() && package.name.as_str() != FIXTURE_PACKAGE)
        .map(|package| package.name.to_string())
        .collect();
    assert_eq!(
        local,
        REQUIRED_DEPENDENCIES
            .iter()
            .map(|name| (*name).to_owned())
            .collect::<std::collections::BTreeSet<String>>(),
        "the restated expectation drifted from the fixture's own graph; update it only together \
         with the declaration"
    );

    let declared: std::collections::BTreeSet<String> = declared_packages()
        .expect("the fixture's closure must resolve")
        .into_iter()
        .collect();
    let missing: Vec<&String> = local.difference(&declared).collect();
    assert!(
        missing.is_empty(),
        "the declaration must cover every workspace crate the fixture compiles; missing \
         {missing:?}. A crate left out of `owned_packages` is filtered out of both the recorded \
         and the present name lists, so an eviction of it passes the gate and the timed suite \
         rebuilds it"
    );
}

/// Evicting any required crate's fingerprint records must fail the gate.
///
/// The declaration is what decides whether an eviction is visible at all: a
/// crate absent from `owned_packages` is filtered out of both the recorded and
/// the present name lists, compares equal to itself, and passes. Each crate is
/// therefore evicted in turn from a profile that has *already* been prepared
/// and stamped, and the gate must refuse every one.
#[test]
fn evicting_each_required_crate_fails_the_gate() -> std::io::Result<()> {
    let declared: std::collections::BTreeSet<String> = declared_packages()?.into_iter().collect();
    for package in REQUIRED_DEPENDENCIES {
        assert!(
            declared.contains(*package),
            "the family must declare {package}: preparing the fixture compiles it, and Cargo \
             rebuilds a crate whose fingerprint records went"
        );
    }

    for package in REQUIRED_DEPENDENCIES {
        let scratch = tempfile::tempdir()?;
        let (preparation, profile) = family_preparation(scratch.path())?;
        nested_cargo::prepare_now(&preparation, || {
            Ok(nested_cargo::PreparationSummary::default())
        })
        .expect("the fixture profile must prepare and stamp");

        let records = profile.join(format!(".fingerprint/{package}-0123456789abcdef"));
        assert!(
            records.is_dir(),
            "the fixture must write {package}'s records"
        );
        fs::remove_dir_all(&records)?;

        let error = nested_cargo::ensure_prepared_with(&preparation, true, || {
            panic!("a timed test must never prepare")
        })
        .expect_err("an evicted dependency must fail the gate");
        let message = error.to_string();
        assert!(
            message.contains(package),
            "the refusal must name {package}, not just the profile: {message}"
        );
    }
    Ok(())
}
