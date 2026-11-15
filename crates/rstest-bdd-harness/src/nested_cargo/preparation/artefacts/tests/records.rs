//! The recorded-name layer: a stamped tree whose units are partly gone.
//!
//! A structural check answers "are the directories Cargo uses still
//! populated". Every case here leaves that answer true and removes something
//! inside one unit: an output, a record directory, a record file, or one of two
//! builds of the same crate. Only the names preparation recorded separate those
//! from a tree Cargo would reuse, so each case fails a gate that consulted the
//! structure alone.

use std::fs;

use super::{preparation_with, profile_of, write_store, write_units};
use crate::nested_cargo::preparation::{
    artefacts::{Eviction, all_present, evicted, record},
    stamp::Stamp,
};

/// A recorded output that was removed is an eviction.
///
/// The case no naming rule can decide: a unit whose `.rlib` went while its
/// `.rmeta` stayed. Cargo recompiles that unit, so the gate must refuse it —
/// and the surviving `.rmeta` is indistinguishable, by name alone, from a
/// genuine metadata-only unit that Cargo does reuse. Only the record of what
/// preparation left separates them.
#[test]
fn a_recorded_output_that_was_removed_is_an_eviction() {
    let (directory, preparation) = preparation_with(&[("unit", "rlib")]);
    // A second, metadata-only unit keeps the profile structurally valid and
    // gives the evicted unit's sibling something to be compared against.
    // Without it, removing `unit`'s last two files would empty the directory
    // and the structural layer would refuse for a reason that has nothing to do
    // with the record.
    let profile = profile_of(&preparation);
    write_units(&profile.join("deps"), &[("checked", "rmeta")]);
    assert!(directory.path().exists(), "the fixture must stay alive");
    let recorded = record(&preparation).expect("record");

    fs::remove_file(profile.join("deps/libunit.rlib")).expect("remove the linked output");
    assert!(
        profile.join("deps/unit.d").is_file(),
        "the record must survive, or the structural layer would refuse instead"
    );
    assert!(
        profile.join("deps/libchecked.rmeta").is_file(),
        "a genuine metadata-only unit must stay accepted beside the eviction"
    );

    let stamp = Stamp { recorded };
    let evictions = evicted(&preparation, Some(&stamp)).expect("check");
    let [Eviction { missing, .. }] = evictions.as_slice() else {
        panic!("exactly one profile must be evicted, got {evictions:?}");
    };
    assert_eq!(
        missing,
        &vec!["deps/libunit.rlib".to_owned()],
        "the message must name the artefact that went, not the metadata that stayed"
    );
}

/// The same eviction when the surviving form is one the unit really did
/// produce: a unit that emitted both an `.rlib` and an `.rmeta`, with only the
/// `.rlib` removed.
///
/// This is the shape a naming rule cannot separate from the previous case, and
/// it is the one a reviewer raised against an earlier revision: if the gate
/// accepted *any* same-stem output, the surviving `.rmeta` would witness the
/// unit and the eviction would pass. Recording the names that preparation
/// actually left is what refuses it, and the message must name the form that
/// went rather than the form that stayed.
#[test]
fn an_rlib_removed_beside_a_surviving_rmeta_is_an_eviction() {
    let (directory, preparation) = preparation_with(&[("unit", "rlib")]);
    let profile = profile_of(&preparation);
    // A unit with both forms, exactly as `rustc` leaves a compiled library that
    // Cargo also kept metadata for.
    fs::write(profile.join("deps/libunit.rmeta"), b"unit").expect("metadata output");
    write_units(&profile.join("deps"), &[("checked", "rmeta")]);
    assert!(directory.path().exists(), "the fixture must stay alive");
    let recorded = record(&preparation).expect("record");
    assert!(
        recorded
            .iter()
            .flatten()
            .any(|name| name == "deps/libunit.rmeta"),
        "preparation recorded the metadata form too, so the eviction cannot be a never-produced \
         case: {recorded:?}"
    );

    fs::remove_file(profile.join("deps/libunit.rlib")).expect("remove the linked output");
    assert!(
        profile.join("deps/libunit.rmeta").is_file(),
        "the surviving form is precisely what must not witness the unit"
    );

    let stamp = Stamp { recorded };
    let evictions = evicted(&preparation, Some(&stamp)).expect("check");
    let [Eviction { missing, .. }] = evictions.as_slice() else {
        panic!("exactly one profile must be evicted, got {evictions:?}");
    };
    assert_eq!(
        missing,
        &vec!["deps/libunit.rlib".to_owned()],
        "a surviving same-stem output must not be read as the unit's witness"
    );
}

/// A unit that never emitted an `.rlib` is not required to have one.
#[test]
fn a_metadata_only_unit_is_accepted() {
    let (_directory, preparation) = preparation_with(&[("checked", "rmeta")]);
    let recorded = record(&preparation).expect("record");
    assert_eq!(
        recorded,
        vec![vec![
            ".fingerprint/unit-0123456789abcdef/lib-unit".to_owned(),
            ".fingerprint/unit-0123456789abcdef/lib-unit.json".to_owned(),
            "deps/libchecked.rmeta".to_owned(),
        ]],
        "only the forms the preparation produced may be vouched for"
    );
    let stamp = Stamp { recorded };
    assert!(
        all_present(&preparation, Some(&stamp)).expect("check"),
        "a crate type that emits only metadata must not be refused"
    );
}

/// A missing profile, a profile with no units, and a profile with no
/// fingerprint store each refuse.
#[test]
fn a_profile_without_both_halves_is_an_eviction() {
    let (_directory, preparation) = preparation_with(&[("unit", "rlib"), ("checked", "rmeta")]);
    assert!(
        all_present(&preparation, None).expect("populated check"),
        "the fixture must start usable"
    );
    let profile = profile_of(&preparation);

    // One unit's record and output go together while a sibling unit keeps the
    // directory non-empty. That is the state the structural layer alone cannot
    // see, and the reason a record of what preparation produced is needed at
    // all.
    fs::remove_file(profile.join("deps/libunit.rlib")).expect("evict");
    fs::remove_file(profile.join("deps/unit.d")).expect("evict record");
    assert!(
        all_present(&preparation, None).expect("structural check"),
        "a unit leaving together with its record leaves a non-empty tree, which the structural \
         layer alone accepts: only a record of what was produced can say more"
    );

    // No units at all, while the fingerprint store survives.
    let units = profile.join("deps");
    let saved = fs::read_dir(&units)
        .expect("list the unit dir")
        .map(|entry| entry.expect("entry").file_name())
        .collect::<Vec<_>>();
    for name in &saved {
        fs::remove_file(units.join(name)).expect("evict the unit");
    }
    assert!(
        !all_present(&preparation, None).expect("check"),
        "a profile holding no units must never pass, however many fingerprints it keeps"
    );
    for name in &saved {
        fs::write(units.join(name), b"restore").expect("restore the unit");
    }

    // No fingerprint store, while the units survive.
    fs::remove_dir_all(profile.join(".fingerprint")).expect("evict the store");
    assert!(
        !all_present(&preparation, None).expect("check"),
        "a profile holding no fingerprint records must never pass"
    );

    fs::remove_dir_all(&profile).expect("evict the profile");
    assert!(
        !all_present(&preparation, None).expect("check"),
        "an absent profile must never pass"
    );
}

/// A unit whose fingerprint record went is an eviction, even though every
/// output it produced is still there and the store still holds another unit's
/// records.
///
/// Cargo decides a unit is current by reading that unit's own record. Removing
/// one record leaves the store populated and the profile structurally usable,
/// and leaves every recorded output in place, so both cheaper layers accept a
/// tree whose next build recompiles that unit and its dependants — inside the
/// timed assertion this preparation exists to keep out of its deadline.
#[test]
fn a_removed_fingerprint_record_is_an_eviction() {
    let (directory, preparation) = preparation_with(&[("unit", "rlib"), ("checked", "rmeta")]);
    let profile = profile_of(&preparation);
    // A second unit's records, so the store stays populated after the first
    // unit's are removed. Without them the store would empty and the
    // structural layer would refuse for a reason that has nothing to do with
    // the record.
    write_store(&profile.join(".fingerprint"), "checked");
    assert!(directory.path().exists(), "the fixture must stay alive");
    let recorded = record(&preparation).expect("record");
    assert!(
        recorded
            .iter()
            .flatten()
            .any(|name| name.ends_with(".fingerprint/unit-0123456789abcdef/lib-unit")),
        "preparation must record the unit's own fingerprint record: {recorded:?}"
    );

    let evicted_records = profile.join(".fingerprint/unit-0123456789abcdef");
    fs::remove_dir_all(&evicted_records).expect("evict one unit's records");
    assert!(
        profile.join("deps/libunit.rlib").is_file() && profile.join("deps/unit.d").is_file(),
        "the unit's outputs must survive, or the output layer would refuse instead"
    );
    assert!(
        profile
            .join(".fingerprint/checked-0123456789abcdef/lib-checked")
            .is_file(),
        "a sibling record must survive, or the structural layer would refuse instead"
    );
    assert!(
        all_present(&preparation, None).expect("structural check"),
        "the store is still populated, so the structural layer alone accepts this tree"
    );

    let stamp = Stamp { recorded };
    let evictions = evicted(&preparation, Some(&stamp)).expect("check");
    let [Eviction { missing, .. }] = evictions.as_slice() else {
        panic!("exactly one profile must be evicted, got {evictions:?}");
    };
    assert!(
        missing
            .iter()
            .all(|name| name.starts_with(".fingerprint/unit-0123456789abcdef/")),
        "the message must name the records that went, got {missing:?}"
    );
    assert!(
        !missing.is_empty(),
        "a removed record must be reported, not silently accepted"
    );
}

/// A removed fingerprint *file*, with its directory left behind, is an
/// eviction.
///
/// The same hole one level down: empty the record directory rather than remove
/// it, and every directory the structural layer names still exists and the
/// store still looks populated from above. Cargo reads the file, so deleting it
/// makes the unit dirty however intact the tree around it is.
#[test]
fn a_removed_fingerprint_file_is_an_eviction() {
    let (_directory, preparation) = preparation_with(&[("unit", "rlib"), ("checked", "rmeta")]);
    let profile = profile_of(&preparation);
    write_store(&profile.join(".fingerprint"), "checked");
    let recorded = record(&preparation).expect("record");

    let record_file = profile.join(".fingerprint/unit-0123456789abcdef/lib-unit");
    fs::remove_file(&record_file).expect("evict the record file");
    assert!(
        record_file.parent().is_some_and(std::path::Path::is_dir),
        "the record directory must survive, so only the file layer can refuse"
    );
    assert!(
        all_present(&preparation, None).expect("structural check"),
        "the store still holds a directory and a sibling unit's records"
    );

    let stamp = Stamp { recorded };
    let evictions = evicted(&preparation, Some(&stamp)).expect("check");
    let [Eviction { missing, .. }] = evictions.as_slice() else {
        panic!("exactly one profile must be evicted, got {evictions:?}");
    };
    assert_eq!(
        missing,
        &vec![".fingerprint/unit-0123456789abcdef/lib-unit".to_owned()],
        "the deletion must be named as the record that went"
    );
}

/// Two recorded builds of one unit: removing one is an eviction, and the
/// surviving sibling must not answer for it.
///
/// A profile can hold one crate compiled twice under different metadata hashes.
/// Recording both output names is what the exact layer does; if one `.rlib` is
/// then removed, reading the other — a name this same stamp recorded — as its
/// rebuild would report a lost unit as present at exactly the moment Cargo
/// recompiles.
#[test]
fn a_recorded_sibling_does_not_answer_for_a_removed_build() {
    let (_directory, preparation) = preparation_with(&[("unit", "rlib")]);
    let profile = profile_of(&preparation);
    // A metadata-only sibling keeps the profile structurally usable once the
    // `.rlib` goes; the sibling under test is the *second build of the same
    // unit*, written with the hash Cargo would give a recompile.
    write_units(&profile.join("deps"), &[("checked", "rmeta")]);
    let sibling = profile.join("deps/libunit-fedcba9876543210.rlib");
    fs::write(&sibling, b"unit").expect("second build");
    // The second build needs its own `.d` record to be discovered as a unit
    // output at record time, exactly as rustc would have left it.
    fs::write(profile.join("deps/unit-fedcba9876543210.d"), b"dep-info")
        .expect("second build record");
    let recorded = record(&preparation).expect("record");
    assert!(
        recorded
            .iter()
            .flatten()
            .any(|name| name == "deps/libunit-fedcba9876543210.rlib"),
        "the second build must itself be recorded: {recorded:?}"
    );

    fs::remove_file(profile.join("deps/libunit.rlib")).expect("remove one build");
    assert!(sibling.is_file(), "the recorded sibling must survive");

    let stamp = Stamp { recorded };
    let evictions = evicted(&preparation, Some(&stamp)).expect("check");
    let [Eviction { missing, .. }] = evictions.as_slice() else {
        panic!("exactly one profile must be evicted, got {evictions:?}");
    };
    assert_eq!(
        missing,
        &vec!["deps/libunit.rlib".to_owned()],
        "a name recorded alongside the missing one must not stand in for it"
    );
}
