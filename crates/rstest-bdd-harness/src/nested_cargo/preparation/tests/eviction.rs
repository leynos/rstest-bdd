//! Evictions: the states a stamp cannot see and the gate must still refuse.
//!
//! A stamp answers "the inputs and environment have not changed". It cannot
//! answer "the compiled units are still there", because the profile directory
//! holding them need not be inside the family's stamp root. Every case here
//! therefore leaves the stamp intact and removes something beneath the
//! profile, which is exactly the state a stamp-only gate would admit and a
//! timed assertion would then pay to rebuild.

use std::{fs, io, path::PathBuf};

use super::{super::stamp::stamp_path, scratch_preparation};

/// The CLI fixture package, and one workspace crate its build also produces.
///
/// The dependency case below turns on the pair: declaring the fixture alone
/// leaves the dependency filtered out of both name lists.
const FIXTURE_PACKAGE: &str = "rstest-bdd-cli-fixture";
const DEPENDENCY_PACKAGE: &str = "rstest-bdd";
use crate::nested_cargo::{
    NestedCargoEnv,
    Preparation,
    PreparationSummary,
    ensure_prepared_with,
    prepare_now,
};

/// The profile directory a scratch preparation vouches for.
///
/// A fixture that vouched for no directory at all would leave every case here
/// unable to name what it evicted, so the absence is reported as `NotFound`
/// rather than panicking inside the shared helper.
fn prepared_profile(preparation: &Preparation) -> io::Result<PathBuf> {
    preparation.artefacts.first().cloned().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::NotFound,
            "the preparation describes the profile directory it reuses",
        )
    })
}

/// A preparation that has already stamped a usable profile.
///
/// Each case below removes something *after* a successful preparation, so the
/// stamp is genuinely written and the eviction is the only thing under test. A
/// fixture that never prepared would be refused for the wrong reason.
fn stamped_preparation() -> io::Result<(tempfile::TempDir, Preparation, NestedCargoEnv)> {
    let (directory, preparation, environment) = scratch_preparation()?;
    prepare_now(&preparation, || Ok(PreparationSummary::default()))?;
    ensure_prepared_with(&preparation, true, || {
        panic!("a just-prepared family must pass")
    })?;
    Ok((directory, preparation, environment))
}

#[test]
fn evicting_only_the_profile_invalidates_the_preparation() {
    // A stamp lives in the family's own scratch root, which is a *sibling* of
    // Cargo's profile directory rather than a descendant: evicting
    // `<target>/debug` removes every compiled dependency unit while leaving
    // the stamp and its scratch root intact. A gate that consulted only the
    // stamp would admit a timed assertion against an empty profile, and that
    // assertion would rebuild the whole graph inside its own deadline.
    let (_directory, preparation, _environment) =
        stamped_preparation().expect("a preparation that has already stamped a usable profile");
    let profile = prepared_profile(&preparation)
        .expect("the preparation describes the profile directory it reuses");

    fs::remove_dir_all(&profile).expect("evict the profile");
    assert!(
        stamp_path(&preparation).is_file(),
        "the stamp must survive an eviction that removes only the profile: that is exactly why \
         the stamp alone cannot answer this question"
    );

    let error = ensure_prepared_with(&preparation, true, || {
        panic!("a timed test must never prepare")
    })
    .expect_err("an evicted profile must fail the gate");
    assert_eq!(error.kind(), io::ErrorKind::NotFound);
    let message = error.to_string();
    assert!(
        message.contains("artefacts it vouched for are gone"),
        "the message must name the evicted artefacts: {message}"
    );
    assert!(message.contains("debug"), "{message}");
}

#[test]
fn evicting_either_required_half_alone_invalidates_the_preparation() {
    // The gate requires two things of a profile: a directory holding compiled
    // units, and a non-empty fingerprint store. They are removed one at a time
    // here. Removing both together would pass even if only one were checked:
    // either half's absence empties the profile check, so the other could be
    // silently ignored. Cargo needs both — the units are what a timed assertion
    // links, and the store holds the records it reads to decide those units are
    // current — so each removal alone must be refused.
    //
    // The pair is named here rather than read from a public constant: which
    // directories Cargo uses is its layout, and this module now discovers them.
    for half in ["units", "fingerprints"] {
        let (_directory, preparation, _environment) =
            stamped_preparation().expect("a preparation that has already stamped a usable profile");
        let profile = prepared_profile(&preparation)
            .expect("the preparation describes the profile directory it reuses");
        let removed = match half {
            "units" => profile.join("deps"),
            _ => profile.join(".fingerprint"),
        };

        fs::remove_dir_all(&removed).expect("evict one required half");
        assert!(profile.is_dir(), "the profile directory must survive");
        assert!(
            stamp_path(&preparation).is_file(),
            "the stamp must survive removing {half}"
        );
        let sibling = match half {
            "units" => profile.join(".fingerprint"),
            _ => profile.join("deps"),
        };
        assert!(
            sibling.is_dir(),
            "only {half} may be missing, so its sibling still has to satisfy its own check"
        );

        let error = ensure_prepared_with(&preparation, true, || {
            panic!("a timed test must never prepare")
        })
        .expect_err("a missing required half must fail the gate");
        assert_eq!(
            error.kind(),
            io::ErrorKind::NotFound,
            "removing {half} must be refused"
        );
        let message = error.to_string();
        assert!(
            message.contains("artefacts it vouched for are gone"),
            "removing {half} must name the eviction: {message}"
        );
        assert!(
            message.contains("debug"),
            "removing {half} must name the profile: {message}"
        );
    }
}

#[test]
fn emptying_a_unit_directory_invalidates_the_preparation() {
    // `cargo clean -p` and a glob that misses both leave the directories in
    // place and remove what matters. An empty directory is not a prepared
    // tree, so the check cannot be `exists()`: it has to count entries.
    let (_directory, preparation, _environment) =
        stamped_preparation().expect("a preparation that has already stamped a usable profile");
    let profile = prepared_profile(&preparation)
        .expect("the preparation describes the profile directory it reuses");
    let deps = profile.join("deps");

    fs::remove_dir_all(&deps).expect("evict the dependency artefacts");
    fs::create_dir_all(&deps).expect("recreate the directory, empty");
    assert!(deps.is_dir(), "the directory must still exist");
    assert_eq!(
        fs::read_dir(&deps)
            .expect("read the empty directory")
            .count(),
        0,
        "the directory must be genuinely empty for this case to mean anything"
    );
    assert!(
        stamp_path(&preparation).is_file(),
        "the stamp must survive an emptied directory"
    );

    let error = ensure_prepared_with(&preparation, true, || {
        panic!("a timed test must never prepare")
    })
    .expect_err("an emptied unit directory must fail the gate");
    assert_eq!(error.kind(), io::ErrorKind::NotFound);
}

#[test]
fn a_dep_info_record_without_its_output_invalidates_the_preparation() {
    // The one eviction the directory checks cannot see: `deps` is still full
    // and `.fingerprint` is untouched, but a unit's output has gone. Cargo
    // recompiles that unit, and a gate that only counted directory entries
    // would admit the rebuild into a timed assertion.
    let (directory, preparation, _environment) =
        stamped_preparation().expect("a preparation that has already stamped a usable profile");
    let profile = prepared_profile(&preparation)
        .expect("the preparation describes the profile directory it reuses");
    let deps = profile.join("deps");

    // Remove ONLY the compiled unit; its dep-info record and every directory
    // the gate names stay exactly where they were. Both stay the same because
    // this removes one file, not a directory.
    fs::remove_file(deps.join("libunit.rlib")).expect("evict the compiled unit");
    assert!(deps.join("unit.d").is_file(), "the dep-info must survive");
    assert!(stamp_path(&preparation).is_file(), "the stamp must survive");
    assert!(
        profile.join("deps").is_dir() && profile.join(".fingerprint").is_dir(),
        "both required halves must survive"
    );

    let error = ensure_prepared_with(&preparation, true, || {
        panic!("a timed test must never prepare")
    })
    .expect_err("a missing unit output must fail the gate");
    assert_eq!(error.kind(), io::ErrorKind::NotFound);
    let message = error.to_string();
    assert!(
        message.contains("artefacts it vouched for are gone"),
        "the message must name the eviction: {message}"
    );
    assert!(
        message.contains("deps/libunit.rlib"),
        "the message must name the artefact that went: {message}"
    );
    assert!(
        directory.path().exists(),
        "the fixture must stay alive for the message"
    );
}

#[test]
fn a_metadata_only_unit_is_paired_with_its_dep_info_record() {
    // The opposite failure to the case above, and the one that matters more:
    // a real `cargo` tree is full of units that left only `lib<stem>.rmeta`,
    // because they were built for `cargo check` or only ever asked for
    // metadata. Measured on this workspace's own `target/debug/deps`: 1,295
    // dep-info records against 1,061 `rmeta` outputs and 739 `rlib` ones. A
    // predicate that accepted only `rlib` refused that genuine tree and named
    // over a hundred units as missing.
    //
    // So: a stamped tree with a metadata-only unit must be accepted, and
    // removing that unit must then be refused. Both halves are needed — the
    // first alone would pass if the record were merely ignored, and the second
    // alone would pass if it were merely counted.
    let (_directory, preparation, _environment) =
        stamped_preparation().expect("a preparation that has already stamped a usable profile");
    let deps = prepared_profile(&preparation)
        .expect("the preparation describes the profile directory it reuses")
        .join("deps");
    assert!(
        deps.join("libchecked.rmeta").is_file(),
        "the fixture must model a metadata-only unit"
    );

    fs::remove_file(deps.join("libchecked.rmeta")).expect("evict the metadata-only unit");
    assert!(
        deps.join("checked.d").is_file(),
        "the dep-info must survive"
    );

    let error = ensure_prepared_with(&preparation, true, || panic!("never prepares"))
        .expect_err("a metadata-only unit that went missing must fail the gate");
    assert_eq!(error.kind(), io::ErrorKind::NotFound);
    let message = error.to_string();
    assert!(
        message.contains("deps/libchecked.rmeta"),
        "the message must name the metadata-only artefact that went: {message}"
    );
}

#[test]
fn an_empty_profile_directory_is_not_a_prepared_family() {
    // `mkdir -p <target>/debug` is not a build. The gate must not read a bare
    // directory as evidence that the dependency graph exists.
    let (directory, preparation, _environment) =
        scratch_preparation().expect("the scratch preparation fixture");
    let profile = prepared_profile(&preparation)
        .expect("the preparation describes the profile directory it reuses");
    fs::remove_dir_all(&profile).expect("evict the populated profile");
    fs::create_dir_all(&profile).expect("recreate an empty profile");

    // The preparation itself now refuses: a command that exits zero while
    // leaving no usable artefacts must not be stamped, because the stamp would
    // advertise a prepared family whose next timed assertion pays for the
    // rebuild.
    let error = prepare_now(&preparation, || Ok(PreparationSummary::default()))
        .expect_err("a preparation that leaves no artefacts must fail");
    assert_eq!(error.kind(), io::ErrorKind::NotFound);
    assert!(
        error.to_string().contains("left no usable artefacts"),
        "{error}"
    );
    // And the gate still refuses the same state, so no stamp was written.
    let error = ensure_prepared_with(&preparation, true, || {
        panic!("a timed test must never prepare")
    })
    .expect_err("an empty profile must fail the gate");
    assert_eq!(error.kind(), io::ErrorKind::NotFound);
    assert!(
        profile.join("deps").symlink_metadata().is_err()
            && profile.join(".fingerprint").symlink_metadata().is_err(),
        "the fixture must leave the profile genuinely empty: {}",
        directory.path().display()
    );
}

#[test]
fn removing_one_units_fingerprint_records_invalidates_the_preparation() {
    // The state `rm -rf <store>/<unit>-<hash>` leaves: every compiled output
    // and every directory survives, and only one unit's records go. Cargo marks
    // a unit whose fingerprint record is missing as dirty and rebuilds it and
    // its dependants, so a gate that recorded only compiled outputs would admit
    // that rebuild into a timed assertion. A sibling unit's records are kept
    // deliberately: the store then still looks populated from above, so the
    // directory-level check cannot be what refuses, and the refusal has to come
    // from the per-unit record check.
    let (_directory, preparation, _environment) =
        stamped_preparation().expect("a preparation that has already stamped a usable profile");
    let profile = prepared_profile(&preparation)
        .expect("the preparation describes the profile directory it reuses");
    let store = profile.join(".fingerprint");
    let mut records: Vec<PathBuf> = fs::read_dir(&store)
        .expect("read the fingerprint store")
        .map(|entry| entry.expect("a store entry").path())
        .collect();
    records.sort();
    assert!(
        records.len() >= 2,
        "the fixture must hold several units' records so a sibling can be kept: {}",
        store.display()
    );

    let removed = records.remove(0);
    fs::remove_dir_all(&removed).expect("evict one unit's fingerprint records");

    // Every layer above the per-unit check must still be satisfied, or the
    // refusal below would prove an ordinary eviction rather than this one.
    assert!(
        removed.symlink_metadata().is_err(),
        "the removed records must be the only thing that went: {}",
        removed.display()
    );
    assert!(
        store.is_dir(),
        "the store itself must survive, so this is not the whole-store case"
    );
    for sibling in &records {
        assert!(
            sibling.is_dir(),
            "a sibling unit's records must survive: {}",
            sibling.display()
        );
    }
    let deps = profile.join("deps");
    assert!(deps.join("unit.d").is_file(), "the dep-info must survive");
    assert!(
        deps.join("libunit.rlib").is_file(),
        "the compiled unit must survive"
    );
    assert!(
        deps.join("libchecked.rmeta").is_file(),
        "the metadata-only unit must survive"
    );
    assert!(stamp_path(&preparation).is_file(), "the stamp must survive");

    let error = ensure_prepared_with(&preparation, true, || {
        panic!("a timed test must never prepare")
    })
    .expect_err("a unit whose records went must fail the gate");
    assert_eq!(error.kind(), io::ErrorKind::NotFound);
    let message = error.to_string();
    assert!(
        message.contains("artefacts it vouched for are gone"),
        "the message must name the eviction: {message}"
    );
    let unit = removed
        .file_name()
        .expect("the evicted record directory has a name")
        .to_string_lossy()
        .into_owned();
    assert!(
        message.contains(&unit),
        "the message must name the unit whose records went, not just the profile: {message}"
    );
}

#[test]
fn evicting_a_dependency_owned_through_the_fixture_invalidates_the_preparation() {
    // The CLI family shares the workspace `target/debug` and therefore declares
    // only its own fixture package, so a name belonging to another package is
    // filtered out of what its stamp records. That protection must stop at the
    // *unrelated sibling*: the fixture's own path dependencies are units its
    // preparation built, and Cargo rebuilds them inside the timed test if their
    // fingerprints go. Recording nothing for them would let this profile — its
    // fixture units and every sibling unit still intact — pass a gate that an
    // evicted `rstest-bdd` should fail.
    let (_directory, mut preparation, _environment) =
        scratch_preparation().expect("the scratch preparation fixture");
    // `scratch_preparation` leaves `owned_packages` empty, which claims the
    // whole profile. Without narrowing it, every name is owned and this case
    // would pass for a reason that has nothing to do with ownership — the
    // profile is shared, and declaring only the fixture is what the CLI family
    // used to do.
    preparation.owned_packages = [FIXTURE_PACKAGE, DEPENDENCY_PACKAGE]
        .into_iter()
        .map(str::to_owned)
        .collect();
    let profile = prepared_profile(&preparation)
        .expect("the preparation describes the profile directory it reuses");
    let store = profile.join(".fingerprint");

    // The fixture's own units, written beside the dependency's records: the
    // profile must still look populated and owned from above, so the refusal
    // has to come from the dependency record check.
    let fixture_unit = store.join(format!("{FIXTURE_PACKAGE}-0123456789abcdef"));
    let dependency_unit = store.join("rstest-bdd-0123456789abcdef");
    for unit in [&fixture_unit, &dependency_unit] {
        fs::create_dir_all(unit).expect("create a unit's record directory");
        fs::write(unit.join("lib-unit"), b"fingerprint record").expect("write a record");
    }

    // The stamp has to exist before the eviction, and it can only be written by
    // a preparation that ran over this tree. Under nextest the gate itself is
    // fail-fast, so the family is prepared through the unconditional entry
    // point — exactly as `make prepare-nested-fixtures` reaches it.
    prepare_now(&preparation, || Ok(PreparationSummary::default()))
        .expect("prepare the narrowed family over its fixture units");
    assert!(
        stamp_path(&preparation).is_file(),
        "the stamp must be written"
    );

    fs::remove_dir_all(&dependency_unit).expect("evict the dependency's fingerprint records");
    assert!(
        fixture_unit.is_dir(),
        "the fixture's own records must survive, or this is not the ownership case"
    );

    let error = ensure_prepared_with(&preparation, true, || {
        panic!("a timed test must never prepare")
    })
    .expect_err("a dependency whose records went must fail the gate");
    assert_eq!(error.kind(), io::ErrorKind::NotFound);
    let message = error.to_string();
    assert!(
        message.contains("rstest-bdd-0123456789abcdef"),
        "the message must name the evicted dependency, not just the profile: {message}"
    );
}
