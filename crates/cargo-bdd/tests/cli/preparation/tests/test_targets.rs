//! The fixture's test targets, as units the stamp and the witness answer for.
//!
//! A package name covers the units that share it, and the fixture's two
//! integration tests do not: Cargo names each for its target. The cases here
//! pin the two consequences. The family has to declare the targets, or the
//! removal of a prepared test binary is invisible to the stamp; and the
//! witness has to read the profile the selection names, or binaries a host
//! build left behind stand in for the ones a selected build reads.

use std::{fs, io, path::Path};

use super::{an_ample_budget, ensure_gated_with, nested_cargo, owned_units, witness};

/// The triple the selected-profile case names.
const TRIPLE: &str = "x86_64-unknown-linux-gnu";

/// Write one profile holding a dependency unit and a fingerprint store.
///
/// The dependency is what keeps the profile structurally usable whatever a
/// case then does to the fixture's own units, so a refusal can only come from
/// the test binaries.
fn write_profile(profile: &Path) -> io::Result<()> {
    let deps = profile.join("deps");
    fs::create_dir_all(&deps)?;
    fs::create_dir_all(profile.join(".fingerprint"))?;
    fs::write(profile.join(".fingerprint/unit"), b"record")?;
    fs::write(deps.join("dep_a-0123456789abcdef.d"), b"dep-info")?;
    fs::write(deps.join("libdep_a-0123456789abcdef.rlib"), b"unit")?;
    Ok(())
}

/// The path Cargo gives the test binary `stem` built with metadata `hash`.
fn binary_path(profile: &Path, stem: &str, hash: &str) -> std::path::PathBuf {
    profile
        .join("deps")
        .join(format!("{stem}-{hash}{}", std::env::consts::EXE_SUFFIX))
}

/// Write the test binary `stem` and the dep-info record that makes it a unit.
fn write_binary(profile: &Path, stem: &str, hash: &str) -> io::Result<()> {
    fs::write(
        profile.join("deps").join(format!("{stem}-{hash}.d")),
        b"dep-info",
    )?;
    fs::write(binary_path(profile, stem, hash), b"binary")
}

/// A scratch family below `scratch`, with `vars` as its nested environment.
fn scratch_family(
    scratch: &Path,
    vars: Vec<(std::ffi::OsString, std::ffi::OsString)>,
) -> io::Result<nested_cargo::Preparation> {
    let environment = nested_cargo::env_from_vars(
        vars,
        Some(&scratch.join("target")),
        nested_cargo::ProfileDestination::Caller,
    )?;
    nested_cargo::Preparation::new(
        nested_cargo::Sources::new("cargo-bdd-cli", "test", "hash"),
        nested_cargo::Directories::new(scratch.join("stamp-root"), scratch.join("stamp-root")),
        environment,
    )
}

#[test]
fn removing_a_prepared_test_binary_beside_another_variant_fails_the_gate() -> io::Result<()> {
    // Two variants of each test binary share the profile, as they do once the
    // fixture has been built under two selections, and the stamp records both.
    // Removing one leaves the other to satisfy the witness, so only the
    // record can report the loss — and it holds the name only when the family
    // declares the target, because no package name matches a test binary.
    let scratch = tempfile::tempdir()?;
    let mut preparation = scratch_family(scratch.path(), Vec::new())?;
    // The production declaration, applied to a closure of one dependency. A
    // literal list here would keep passing for a family that had stopped
    // naming its targets.
    preparation.owned_packages = owned_units(vec!["dep_a".to_owned()]);
    let profile = scratch
        .path()
        .join("target")
        .join(nested_cargo::DEFAULT_PROFILE);
    write_profile(&profile)?;
    let (prepared, other) = ("0123456789abcdef", "fedcba9876543210");
    for stem in witness::FIXTURE_TEST_BINARIES {
        write_binary(&profile, stem, prepared)?;
        write_binary(&profile, stem, other)?;
    }
    ensure_gated_with(&preparation, false, an_ample_budget(), || {
        Ok(nested_cargo::PreparationSummary::default())
    })
    .expect("a profile holding every declared unit must prepare and stamp");

    let removed = witness::FIXTURE_TEST_BINARIES[0];
    fs::remove_file(binary_path(&profile, removed, prepared))?;

    let error = ensure_gated_with(&preparation, true, an_ample_budget(), || {
        panic!("a timed test must never prepare")
    })
    .expect_err("a removed test binary must fail the gate while its other variant survives");
    let rendered = format!("{error:?}");
    assert!(
        rendered.contains(removed),
        "the refusal must name the removed test binary, got: {rendered}"
    );
    Ok(())
}

#[test]
fn a_selected_target_is_witnessed_in_its_own_profile() -> io::Result<()> {
    // A host build left both test binaries in `<target>/debug`. The selected
    // build reads `<target>/<triple>/debug`, where one of them is missing, so
    // the host copies are not evidence of anything the timed command will use.
    let scratch = tempfile::tempdir()?;
    let preparation = scratch_family(
        scratch.path(),
        vec![("CARGO_BUILD_TARGET".into(), TRIPLE.into())],
    )?;
    let target = scratch.path().join("target");
    let host = target.join(nested_cargo::DEFAULT_PROFILE);
    let selected = target.join(TRIPLE).join(nested_cargo::DEFAULT_PROFILE);
    write_profile(&host)?;
    write_profile(&selected)?;
    let hash = "0123456789abcdef";
    for stem in witness::FIXTURE_TEST_BINARIES {
        write_binary(&host, stem, hash)?;
    }
    write_binary(&selected, witness::FIXTURE_TEST_BINARIES[1], hash)?;

    let error = ensure_gated_with(&preparation, false, an_ample_budget(), || {
        Ok(nested_cargo::PreparationSummary::default())
    })
    .expect_err("host binaries must not witness a selected target that lacks one");
    let rendered = format!("{error:?}");
    assert!(
        rendered.contains(TRIPLE),
        "the refusal must name the selected profile, got: {rendered}"
    );

    // The positive control: the same tree is admitted once the selected
    // profile holds both, so the refusal above was about that profile.
    write_binary(&selected, witness::FIXTURE_TEST_BINARIES[0], hash)?;
    ensure_gated_with(&preparation, false, an_ample_budget(), || {
        Ok(nested_cargo::PreparationSummary::default())
    })
    .expect("a selected profile holding both test binaries must satisfy the gate");
    Ok(())
}
