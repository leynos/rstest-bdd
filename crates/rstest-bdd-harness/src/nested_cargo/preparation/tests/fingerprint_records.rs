//! Fingerprint-record evictions: the states a stamp and a directory listing
//! both accept.
//!
//! Cargo decides a unit is current by reading that unit's own fingerprint
//! record. Every case here removes those records while leaving the compiled
//! outputs, the record's directory, and the rest of the store in place, so the
//! structural layer still sees a populated profile. Splitting them out keeps
//! `eviction` to the shapes a directory listing can already decide.

use std::{fs, io, path::PathBuf};

use super::{super::stamp::stamp_path, eviction::stamped_preparation, prepared_profile};
use crate::nested_cargo::ensure_prepared_building;

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

    let error = ensure_prepared_building(&preparation, true, || {
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

/// Evicting a record's *files* is refused too, with its directory surviving.
///
/// Distinct from the case above, which removes the record directory: here the
/// per-unit directory stays, so `record_directories` still lists it and the
/// store still looks populated at every level the walk descends through. Only
/// the names recorded from inside it can say the unit's records went. A check
/// that recorded a record's directory rather than its files — or that treated
/// a listed directory as evidence its contents survive — passes this state and
/// lets Cargo rebuild the unit inside the timed assertion.
#[test]
fn removing_a_units_record_files_leaves_its_directory_and_is_still_an_eviction() {
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

    let emptied = records.remove(0);
    for file in fs::read_dir(&emptied).expect("list the unit's records") {
        let file = file.expect("a record entry").path();
        fs::remove_file(file).expect("evict the record file");
    }

    // The directory the walk descends into is still there and still listed,
    // which is the whole of what this case exists to prove: the refusal cannot
    // come from a missing directory.
    assert!(
        emptied.is_dir(),
        "the record directory must survive, so this is not the directory case: {}",
        emptied.display()
    );
    assert!(
        fs::read_dir(&emptied)
            .expect("list the emptied directory")
            .next()
            .is_none(),
        "the directory must be empty, so nothing inside it can satisfy the check"
    );
    assert!(store.is_dir(), "the store itself must survive");
    assert!(
        profile.join("deps/libunit.rlib").is_file(),
        "the compiled unit must survive"
    );
    assert!(stamp_path(&preparation).is_file(), "the stamp must survive");

    let error = ensure_prepared_building(&preparation, true, || {
        panic!("a timed test must never prepare")
    })
    .expect_err("a unit whose record files went must fail the gate");
    assert_eq!(error.kind(), io::ErrorKind::NotFound);
    let message = error.to_string();
    assert!(
        message.contains("artefacts it vouched for are gone"),
        "the message must name the eviction: {message}"
    );
    assert!(
        message.contains("fingerprint"),
        "the message must name a record that went, which is what this case evicts: {message}"
    );
}
