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
use crate::nested_cargo::{
    NestedCargoEnv,
    Preparation,
    PreparationSummary,
    ensure_prepared_with,
    prepare_now,
};

/// The profile directory a scratch preparation vouches for.
fn prepared_profile(preparation: &Preparation) -> PathBuf {
    preparation
        .artefacts
        .first()
        .expect("the preparation describes the profile directory it reuses")
        .clone()
}

/// A preparation that has already stamped a usable profile.
///
/// Each case below removes something *after* a successful preparation, so the
/// stamp is genuinely written and the eviction is the only thing under test. A
/// fixture that never prepared would be refused for the wrong reason.
fn stamped_preparation() -> (tempfile::TempDir, Preparation, NestedCargoEnv) {
    let (directory, preparation, environment) = scratch_preparation();
    prepare_now(&preparation, || Ok(PreparationSummary::default())).expect("preparation");
    ensure_prepared_with(&preparation, true, || {
        panic!("a just-prepared family must pass")
    })
    .expect("the freshly written stamp must be accepted");
    (directory, preparation, environment)
}

#[test]
fn evicting_only_the_profile_invalidates_the_preparation() {
    // A stamp lives in the family's own scratch root, which is a *sibling* of
    // Cargo's profile directory rather than a descendant: evicting
    // `<target>/debug` removes every compiled dependency unit while leaving
    // the stamp and its scratch root intact. A gate that consulted only the
    // stamp would admit a timed assertion against an empty profile, and that
    // assertion would rebuild the whole graph inside its own deadline.
    let (_directory, preparation, _environment) = stamped_preparation();
    let profile = prepared_profile(&preparation);

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
        let (_directory, preparation, _environment) = stamped_preparation();
        let profile = prepared_profile(&preparation);
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
    let (_directory, preparation, _environment) = stamped_preparation();
    let profile = prepared_profile(&preparation);
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
    let (directory, preparation, _environment) = stamped_preparation();
    let profile = prepared_profile(&preparation);
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
    let (_directory, preparation, _environment) = stamped_preparation();
    let deps = prepared_profile(&preparation).join("deps");
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
    let (directory, preparation, _environment) = scratch_preparation();
    let profile = prepared_profile(&preparation);
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
