//! The second profile a `--target <triple>` build leaves behind.
//!
//! `trybuild` passes `--target <triple>` for every generated crate, so a family
//! whose stamped root is the child's `CARGO_TARGET_DIR` holds the host units in
//! `<root>/debug` and every other unit in `<root>/<triple>/debug`. Both are part
//! of one prepared tree, and the cases here need that shape to be stated at all:
//! evicting one profile while the other survives, and repairing the evicted one
//! without trusting the stamp that describes it.

use std::{fs, io};

use super::super::stamp::stamp_path;
use crate::nested_cargo::{
    DEFAULT_PROFILE,
    Preparation,
    PreparationSummary,
    ensure_prepared_with,
    prepare_now,
};

/// Write a second profile the way a `--target <triple>` build leaves it.
///
/// `trybuild` passes `--target <triple>` for every generated crate, so a
/// preparation that runs it leaves the host units in `<root>/debug` and every
/// other unit in `<root>/<triple>/debug`. Both are part of one prepared tree.
/// Write the host profile a `--target <triple>` build also leaves behind.
///
/// The twin of [`write_target_profile`], and shared by both cases for the same
/// reason: the two tests have to describe one tree. A profile is a unit
/// directory and a fingerprint store, and a copy of that shape drifting in one
/// case would silently test something other than the other.
fn write_host_profile(root: &std::path::Path) -> io::Result<std::path::PathBuf> {
    let host = root.join(DEFAULT_PROFILE);
    let deps = host.join("deps");
    fs::create_dir_all(&deps)?;
    fs::create_dir_all(host.join(".fingerprint"))?;
    fs::write(host.join(".fingerprint/unit"), b"fingerprint record")?;
    fs::write(deps.join("unit.d"), b"dep-info")?;
    fs::write(deps.join("libunit.rlib"), b"unit")?;
    Ok(host)
}

fn write_target_profile(root: &std::path::Path, triple: &str) -> io::Result<()> {
    let profile = root.join(triple).join("debug");
    let deps = profile.join("deps");
    fs::create_dir_all(&deps)?;
    fs::create_dir_all(profile.join(".fingerprint"))?;
    fs::write(profile.join(".fingerprint/unit"), b"fingerprint record")?;
    fs::write(deps.join("target_unit.d"), b"dep-info")?;
    fs::write(deps.join("libtarget_unit.rlib"), b"unit")?;
    Ok(())
}

#[test]
fn evicting_only_the_target_profile_invalidates_the_preparation() {
    // The crate whose target directory a family owns is built by `trybuild`,
    // which passes `--target <triple>`. That build writes two profiles:
    // `<root>/debug` for the host units, and `<root>/<triple>/debug` for
    // everything else. Removing only the second leaves the first, and the
    // stamp, exactly where they were, so a gate that named one profile would
    // admit a timed assertion whose graph is half evicted and which pays for
    // the rebuild inside its own deadline.
    let directory = tempfile::tempdir().expect("fixture dir");
    let root = directory.path().join("trybuild-root");
    let host = write_host_profile(&root).expect("host profile");
    write_target_profile(&root, "x86_64-unknown-linux-gnu").expect("target profile");

    let preparation = Preparation::for_fixture("unit-fixture", root.clone(), "v", "hash")
        .expect("the fixture preparation should describe its artefacts");
    prepare_now(&preparation, || Ok(PreparationSummary::default())).expect("preparation");
    ensure_prepared_with(&preparation, true, || {
        panic!("a just-prepared family must pass")
    })
    .expect("the freshly written stamp must accept both profiles");

    let target_profile = root.join("x86_64-unknown-linux-gnu/debug");
    fs::remove_dir_all(&target_profile).expect("evict the target-profile artefacts");
    assert!(host.is_dir(), "the host profile must survive");
    assert!(stamp_path(&preparation).is_file(), "the stamp must survive");

    let error = ensure_prepared_with(&preparation, true, || {
        panic!("a timed test must never prepare")
    })
    .expect_err("an evicted target profile must fail the gate");
    assert_eq!(error.kind(), io::ErrorKind::NotFound);
    let message = error.to_string();
    assert!(
        message.contains("artefacts it vouched for are gone"),
        "the message must name the eviction: {message}"
    );
    assert!(
        message.contains("x86_64-unknown-linux-gnu"),
        "the message must name the profile that went, not only the one that survived: {message}"
    );
}

#[test]
fn the_advertised_preparation_repairs_an_evicted_target_profile() {
    // The stamp is written before any witness can read what was produced, so a
    // matching stamp alone would let `prepare_now` return early and advertise a
    // preparation that leaves the tree exactly as it found it — half evicted.
    // The gate has to re-run the build, and the stamp it then writes has to
    // accept the repaired tree.
    let directory = tempfile::tempdir().expect("fixture dir");
    let root = directory.path().join("trybuild-root");
    write_host_profile(&root).expect("host profile");
    write_target_profile(&root, "x86_64-unknown-linux-gnu").expect("target profile");

    let preparation = Preparation::for_fixture("unit-fixture", root.clone(), "v", "hash")
        .expect("the fixture preparation should describe its artefacts");
    prepare_now(&preparation, || Ok(PreparationSummary::default())).expect("first preparation");
    fs::remove_dir_all(root.join("x86_64-unknown-linux-gnu/debug"))
        .expect("evict the target-profile artefacts");

    // The advertised command must rebuild rather than trust the stamp. Whether
    // it did is recorded by the closure itself: a gate that returned early
    // would leave the target profile absent, and the test would pass on the
    // strength of a host profile the repair never touched.
    let prepared = std::cell::Cell::new(false);
    prepare_now(&preparation, || {
        prepared.set(true);
        write_target_profile(&root, "x86_64-unknown-linux-gnu")?;
        Ok(PreparationSummary::default())
    })
    .expect("preparing must repair the evicted target profile");
    assert!(
        prepared.get(),
        "the advertised preparation must run the build, not return early on a matching stamp"
    );
    assert!(
        root.join("x86_64-unknown-linux-gnu/debug/deps/libtarget_unit.rlib")
            .is_file(),
        "the repair must leave the evicted profile populated"
    );

    ensure_prepared_with(&preparation, true, || {
        panic!("the repaired family must be admitted without preparing again")
    })
    .expect("the stamp written after the repair must accept the repaired tree");
}
