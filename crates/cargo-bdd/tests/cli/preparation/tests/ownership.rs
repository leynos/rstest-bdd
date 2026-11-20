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

use super::{FIXTURE_PACKAGE, nested_cargo};
use crate::preparation::closure::{TIMED_GRAPH_QUERY_TIMEOUT, fixture_dependency_closure};

/// The family's declared packages, with the resolution error as an `io::Error`.
///
/// [`fixture_dependency_closure`] reports through `eyre`, which the shared
/// preparation fixtures in this module cannot return. The resolution failure
/// is the only error either can produce, so it is carried across as an
/// [`std::io::ErrorKind::Other`] error rather than the callers growing an
/// error type that names nothing further.
fn declared_packages(deadline: nested_cargo::Deadline) -> std::io::Result<Vec<String>> {
    // The timed path's bound, not the preparation's: every case here runs in
    // the `cargo-bdd::cli` binary under its 180 s period, and the query takes
    // the package-cache lock without the gate having read a stamp first.
    //
    // Charged against the caller's deadline rather than given a fresh limit.
    // One case here resolves the closure *and* runs `cargo metadata`, and both
    // take the same package-cache lock; a per-call limit let the two run for
    // their sum and reach the runner's kill, which is the failure the shared
    // budget exists to prevent. `cap` also refuses outright when the earlier
    // command spent the budget, so the second is declined rather than started
    // with no time to finish.
    let bound = deadline.cap(
        crate::preparation::closure::TIMED_GRAPH_QUERY_TIMEOUT,
        "the fixture dependency closure",
    )?;
    fixture_dependency_closure(bound).map_err(|error| std::io::Error::other(error.to_string()))
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
/// The declaration is resolved by the caller and threaded through rather than
/// read here: the closure costs one `cargo tree` subprocess, which takes the
/// package-cache lock. Resolving it per iteration would repeat that wait once
/// for every crate the caller evicts, against a budget the case shares with the
/// rest of the suite. The declaration cannot change between iterations — it is
/// the same fixture, resolved offline and locked — so it is resolved once.
fn family_preparation(
    scratch: &std::path::Path,
    packages: Vec<String>,
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
        nested_cargo::Directories::new(scratch.join("stamp-root"), scratch.join("stamp-root")),
        environment,
    )?;
    preparation.owned_packages = packages;

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
///
/// The query is bounded, because this case runs in the timed binary. `.config/
/// nextest.toml` gives `cargo-bdd::cli` 180 s, and `cargo metadata` takes the
/// same package-cache lock `cargo tree` does, so an unbounded call here would
/// let a busy cache kill the test instead of failing it with a diagnosis. The
/// bound is the timed path's own, and it is applied to the metadata process
/// rather than to the case as a whole so the error names the query.
///
/// # Errors
///
/// Returns the query's timeout or spawn error, and the resolution error when
/// the fixture manifest does not resolve.
fn fixture_metadata(
    manifest: &std::path::Path,
    deadline: nested_cargo::Deadline,
) -> Result<cargo_metadata::Metadata, std::io::Error> {
    // Built through `cargo_metadata` rather than by spelling the invocation
    // out here: the crate owns the argument shape and the JSON decoding, and
    // the only thing this case adds is the bound. `cargo_command` and `parse`
    // are the two halves `exec` joins, so running them separately keeps the
    // result identical to `exec`'s apart from the timeout.
    let mut command = cargo_metadata::MetadataCommand::new();
    command
        .manifest_path(manifest)
        // The fixture commits its lockfile, so resolution needs no registry:
        // asserting that keeps a test of the declaration from reaching the
        // network, where a registry hiccup would read as a broken graph.
        .other_options(vec!["--locked".to_owned(), "--offline".to_owned()]);
    let mut cargo = command.cargo_command();
    let bound = deadline.cap(
        TIMED_GRAPH_QUERY_TIMEOUT,
        "`cargo metadata` for the fixture",
    )?;
    let output = nested_cargo::run_bounded_with_timeout(&mut cargo, bound).map_err(|error| {
        std::io::Error::new(
            error.kind(),
            format!(
                "`cargo metadata` for {} did not finish within {} s: {error}",
                manifest.display(),
                TIMED_GRAPH_QUERY_TIMEOUT.as_secs()
            ),
        )
    })?;
    if !output.status.success() {
        return Err(std::io::Error::other(format!(
            "`cargo metadata` failed for {}:\n{}",
            manifest.display(),
            String::from_utf8_lossy(&output.stderr)
        )));
    }
    let stdout = std::str::from_utf8(&output.stdout).map_err(|error| {
        std::io::Error::other(format!("`cargo metadata` emitted no UTF-8: {error}"))
    })?;
    // `exec` reads the first line beginning with `{` from the same stream, so
    // this mirrors it rather than parsing the whole capture.
    let json = stdout
        .lines()
        .find(|line| line.starts_with('{'))
        .ok_or_else(|| std::io::Error::other("`cargo metadata` printed no JSON object"))?;
    cargo_metadata::MetadataCommand::parse(json).map_err(std::io::Error::other)
}

#[test]
fn the_declaration_covers_the_fixtures_workspace_dependencies() {
    let manifest =
        super::super::workspace_root().join("crates/cargo-bdd/tests/fixtures/minimal/Cargo.toml");
    // One budget for both commands, created before the first one runs. They
    // share a package-cache lock and each charged its own full limit put their
    // sum past the runner's period, which is the case the shared deadline
    // exists for. The budget is the timed path's own, so a command still
    // reports its timeout rather than the runner killing the test.
    let deadline = nested_cargo::Deadline::starting_now(TIMED_GRAPH_QUERY_TIMEOUT);
    let metadata =
        fixture_metadata(&manifest, deadline).expect("the fixture manifest must resolve");
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

    // The same budget the metadata query was charged against, so the closure
    // resolution is bounded by what is left rather than by a second full
    // allowance, and is declined by name if the query spent it all.
    let declared: std::collections::BTreeSet<String> = declared_packages(deadline)
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
    // Resolved once: `cargo tree` takes the package-cache lock, and the
    // declaration is the same for every iteration. This case runs no other
    // command first, so its deadline covers the one query.
    let packages = declared_packages(nested_cargo::Deadline::starting_now(
        TIMED_GRAPH_QUERY_TIMEOUT,
    ))?;
    let declared: std::collections::BTreeSet<&str> = packages.iter().map(String::as_str).collect();
    for package in REQUIRED_DEPENDENCIES {
        assert!(
            declared.contains(package),
            "the family must declare {package}: preparing the fixture compiles it, and Cargo \
             rebuilds a crate whose fingerprint records went"
        );
    }

    for package in REQUIRED_DEPENDENCIES {
        let scratch = tempfile::tempdir()?;
        let (preparation, profile) = family_preparation(scratch.path(), packages.clone())?;
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

        let error = nested_cargo::ensure_prepared_building(&preparation, true, || {
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
