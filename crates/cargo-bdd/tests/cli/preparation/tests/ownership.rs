//! The ownership list this family declares, exercised against the real list.
//!
//! [`super::FIXTURE_PATH_DEPENDENCIES`] is a *declaration*, and the harness's
//! ownership tests can only prove the mechanism works for whichever list a
//! case supplies. Neither half on its own catches a list that has drifted from
//! the fixture's actual dependency graph: the mechanism stays correct while
//! the declaration quietly omits a crate the build compiles. This case drives
//! the list the family actually uses, so a missing entry fails here rather
//! than as a rebuild inside a timed assertion.
//!
//! Most of the crates named are *transitive* dependencies. Only `rstest-bdd`
//! and `rstest-bdd-macros` appear in the fixture's manifest; `rstest-bdd`
//! pulls in `rstest-bdd-patterns` and `rstest-bdd-policy`, and
//! `rstest-bdd-macros` pulls in those two plus `rstest-bdd-harness`. A list
//! built from the manifest alone covers the direct edges and leaves the rest
//! unprotected, which is the defect this case exists to catch one level down.

use std::fs;

use super::{FIXTURE_PACKAGE, FIXTURE_PATH_DEPENDENCIES, nested_cargo};

/// A scratch preparation carrying the real family's ownership list.
///
/// The harness's own fixtures live in another crate, so this builds the shape
/// here: one profile below a target root holding compiled units and a
/// fingerprint store, and a stamp root of its own. The ownership list is the
/// family's, not a copy of it — that is the whole point of the case.
fn family_preparation(
    scratch: &std::path::Path,
) -> (nested_cargo::Preparation, std::path::PathBuf) {
    let target = scratch.join("target");
    let profile = target.join(nested_cargo::DEFAULT_PROFILE);
    let deps = profile.join("deps");
    fs::create_dir_all(&deps).expect("deps dir");
    let store = profile.join(".fingerprint");
    fs::create_dir_all(&store).expect("store dir");

    // One unit and one record set per crate the family claims, named the way
    // Cargo names them: a hyphenated record directory, and an underscored
    // library output. Both carry the same 16-hex-digit metadata hash, which is
    // what pairs an output with the record that keeps it current.
    for package in std::iter::once(FIXTURE_PACKAGE).chain(FIXTURE_PATH_DEPENDENCIES.iter().copied())
    {
        let stem = package.replace('-', "_");
        let unit = format!("{stem}-0123456789abcdef");
        fs::write(deps.join(format!("{unit}.d")), b"dep-info").expect("dep-info");
        fs::write(deps.join(format!("lib{unit}.rlib")), b"unit").expect("output");
        let records = store.join(format!("{package}-0123456789abcdef"));
        fs::create_dir_all(&records).expect("record dir");
        fs::write(records.join(format!("lib-{package}")), b"record").expect("record");
    }

    let environment = nested_cargo::env_from_vars(
        Vec::new(),
        Some(&target),
        nested_cargo::ProfileDestination::Caller,
    )
    .expect("the fixture environment should resolve");
    let mut preparation = nested_cargo::Preparation::new(
        nested_cargo::Sources::new("cargo-bdd-cli", "test", "hash"),
        scratch.join("stamp-root"),
        environment,
    )
    .expect("the fixture preparation should describe its artefacts");
    preparation.owned_packages = std::iter::once(FIXTURE_PACKAGE)
        .chain(FIXTURE_PATH_DEPENDENCIES.iter().copied())
        .map(str::to_owned)
        .collect();
    (preparation, profile)
}

/// The crates this family's preparation must vouch for, named independently.
///
/// Deliberately *not* [`FIXTURE_PATH_DEPENDENCIES`]. A case that iterated the
/// declaration would inherit every omission it is meant to catch: drop a crate
/// from the list and the loop simply stops checking it, so the case passes
/// whether or not the defect is present. That was the first version of this
/// file, and reverting the declaration to the two directly-named crates left
/// it green.
///
/// So the expectation is written out again, from the fixture's dependency
/// graph rather than from the constant. The duplication is the point: the two
/// can disagree, and when they do this case fails.
///
/// Every entry but the first two is transitive. The fixture's manifest names
/// only `rstest-bdd` and `rstest-bdd-macros`; `rstest-bdd` pulls in
/// `rstest-bdd-patterns` and `rstest-bdd-policy`, and `rstest-bdd-macros`
/// pulls in those two plus `rstest-bdd-harness`. All five are compiled by the
/// preparation, so all five have to be declared.
const REQUIRED_DEPENDENCIES: &[&str] = &[
    "rstest-bdd",
    "rstest-bdd-macros",
    "rstest-bdd-harness",
    "rstest-bdd-patterns",
    "rstest-bdd-policy",
];

/// Evicting any required crate's fingerprint records must fail the gate.
///
/// The declaration is what decides whether an eviction is visible at all: a
/// crate absent from `owned_packages` is filtered out of both the recorded and
/// the present name lists, compares equal to itself, and passes. Each crate is
/// therefore evicted in turn from a profile that has *already* been prepared
/// and stamped, and the gate must refuse every one.
#[test]
fn evicting_each_required_crate_fails_the_gate() {
    for package in REQUIRED_DEPENDENCIES {
        assert!(
            FIXTURE_PATH_DEPENDENCIES.contains(package),
            "the family must declare {package}: preparing the fixture compiles it, and Cargo \
             rebuilds a crate whose fingerprint records went. Declared: \
             {FIXTURE_PATH_DEPENDENCIES:?}"
        );
    }

    for package in REQUIRED_DEPENDENCIES {
        let scratch = tempfile::tempdir().expect("temp dir");
        let (preparation, profile) = family_preparation(scratch.path());
        nested_cargo::prepare_now(&preparation, || {
            Ok(nested_cargo::PreparationSummary::default())
        })
        .expect("the fixture profile must prepare and stamp");

        let records = profile.join(format!(".fingerprint/{package}-0123456789abcdef"));
        assert!(
            records.is_dir(),
            "the fixture must write {package}'s records"
        );
        fs::remove_dir_all(&records).expect("evict the crate's fingerprint records");

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
}
