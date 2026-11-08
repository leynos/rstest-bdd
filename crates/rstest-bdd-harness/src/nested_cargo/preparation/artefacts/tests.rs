//! Unit tests for the recorded-name check and its structural layer.

use std::fs;

use super::{Eviction, all_present, evicted, is_output_of, record};
use crate::nested_cargo::{Preparation, ProfileDestination, Sources, env_from_vars};

/// The profile directory a `preparation_with` fixture vouches for.
///
/// The fixture always creates exactly one, so a missing one is a broken
/// fixture rather than a case under test.
fn profile_of(preparation: &Preparation) -> std::path::PathBuf {
    preparation
        .artefacts
        .first()
        .expect("the fixture describes one profile")
        .clone()
}

/// Write a profile holding `units`, each as a `.d` record plus its output.
fn preparation_with(units: &[(&str, &str)]) -> (tempfile::TempDir, Preparation) {
    let directory = tempfile::tempdir().expect("temp dir");
    let target = directory.path().join("target");
    let profile = target.join("debug");
    for name in ["deps", ".fingerprint"] {
        fs::create_dir_all(profile.join(name)).expect("dependency dir");
    }
    fs::write(profile.join(".fingerprint/record"), b"record").expect("fingerprint");
    for (stem, extension) in units {
        fs::write(profile.join("deps").join(format!("{stem}.d")), b"dep-info").expect("record");
        fs::write(
            profile.join("deps").join(format!("lib{stem}.{extension}")),
            b"unit",
        )
        .expect("output");
    }
    let environment = env_from_vars(Vec::new(), Some(&target), ProfileDestination::Caller);
    let preparation = Preparation::new(
        Sources::new("unit", "v", "hash"),
        directory.path().to_path_buf(),
        environment,
    );
    (directory, preparation)
}

/// Only same-stem entries pair, and a record is never its own output.
#[test]
fn pairing_is_by_stem_with_a_separator() {
    for output in ["libunit.rlib", "libunit.rmeta", "unit", "unit.exe"] {
        assert!(is_output_of(output, "unit"), "{output} belongs to unit");
    }
    for unrelated in [
        "libunit_extra.rlib",
        "unit-2",
        "unit_extra",
        "libunits.rlib",
    ] {
        assert!(
            !is_output_of(unrelated, "unit"),
            "{unrelated} belongs to another unit"
        );
    }
    assert!(
        !is_output_of("unit.d", "unit"),
        "a record must not witness itself"
    );
}

/// The case no naming rule can decide: a unit whose `.rlib` went while its
/// `.rmeta` stayed.
///
/// Cargo recompiles that unit, so the gate must refuse it — and the
/// `.rmeta` that remains is indistinguishable, by name alone, from a
/// genuine metadata-only unit that Cargo does reuse. Only the record of
/// what preparation left separates them.
#[test]
fn a_recorded_output_that_was_removed_is_an_eviction() {
    let (directory, preparation) = preparation_with(&[("unit", "rlib")]);
    // A second, metadata-only unit keeps `deps` valid on the structural
    // layer's own terms and gives the evicted unit's sibling something to
    // be compared against. Without it, removing `unit`'s last two files
    // would empty the directory and the structural layer would refuse for
    // a reason that has nothing to do with the record.
    let profile = profile_of(&preparation);
    fs::write(profile.join("deps/checked.d"), b"dep-info").expect("record");
    fs::write(profile.join("deps/libchecked.rmeta"), b"unit").expect("output");
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

    let stamp = super::Stamp { recorded };
    let evictions = evicted(&preparation, Some(&stamp)).expect("check");
    let [Eviction { missing, .. }] = evictions.as_slice() else {
        panic!("exactly one profile must be evicted, got {evictions:?}");
    };
    assert_eq!(
        missing,
        &vec!["libunit.rlib".to_owned()],
        "the message must name the artefact that went, not the metadata that stayed"
    );
}

/// The same eviction when the surviving form is one the unit really did
/// produce: a unit that emitted both an `.rlib` and an `.rmeta`, with only the
/// `.rlib` removed.
///
/// This is the shape a naming rule cannot separate from the previous case,
/// and it is the one a reviewer raised against an earlier revision: if the
/// gate accepted *any* same-stem output, the surviving `.rmeta` would witness
/// the unit and the eviction would pass. Recording the names that preparation
/// actually left is what refuses it, and the message must name the form that
/// went rather than the form that stayed.
#[test]
fn an_rlib_removed_beside_a_surviving_rmeta_is_an_eviction() {
    let (directory, preparation) = preparation_with(&[("unit", "rlib")]);
    let profile = profile_of(&preparation);
    // A unit with both forms, exactly as `rustc` leaves a compiled library
    // that Cargo also kept metadata for.
    fs::write(profile.join("deps/libunit.rmeta"), b"unit").expect("metadata output");
    fs::write(profile.join("deps/checked.d"), b"dep-info").expect("record");
    fs::write(profile.join("deps/libchecked.rmeta"), b"unit").expect("output");
    assert!(directory.path().exists(), "the fixture must stay alive");
    let recorded = record(&preparation).expect("record");
    assert!(
        recorded
            .iter()
            .flatten()
            .any(|name| name == "libunit.rmeta"),
        "preparation recorded the metadata form too, so the eviction cannot be a never-produced \
         case: {recorded:?}"
    );

    fs::remove_file(profile.join("deps/libunit.rlib")).expect("remove the linked output");
    assert!(
        profile.join("deps/libunit.rmeta").is_file(),
        "the surviving form is precisely what must not witness the unit"
    );

    let stamp = super::Stamp { recorded };
    let evictions = evicted(&preparation, Some(&stamp)).expect("check");
    let [Eviction { missing, .. }] = evictions.as_slice() else {
        panic!("exactly one profile must be evicted, got {evictions:?}");
    };
    assert_eq!(
        missing,
        &vec!["libunit.rlib".to_owned()],
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
        vec![vec!["libchecked.rmeta".to_owned()]],
        "only the forms the preparation produced may be vouched for"
    );
    let stamp = super::Stamp { recorded };
    assert!(
        all_present(&preparation, Some(&stamp)).expect("check"),
        "a crate type that emits only metadata must not be refused"
    );
}

/// A missing or emptied profile refuses, with or without a record.
#[test]
fn a_missing_profile_is_an_eviction_without_a_record() {
    let (_directory, preparation) = preparation_with(&[("unit", "rlib"), ("checked", "rmeta")]);
    assert!(
        all_present(&preparation, None).expect("populated check"),
        "the fixture must start usable"
    );
    // One unit's record and output go together while a sibling unit keeps
    // `deps` non-empty. That is the state the structural layer alone
    // cannot see, and the reason a record of what preparation produced is
    // needed at all.
    let profile = profile_of(&preparation);
    fs::remove_file(profile.join("deps/libunit.rlib")).expect("evict");
    fs::remove_file(profile.join("deps/unit.d")).expect("evict record");
    assert!(
        all_present(&preparation, None).expect("structural check"),
        "a unit leaving together with its record leaves a non-empty tree, which the structural \
         layer alone accepts: only a record of what was produced can say more"
    );
    fs::remove_dir_all(&profile).expect("evict the profile");
    assert!(
        !all_present(&preparation, None).expect("check"),
        "an absent profile must never pass"
    );
}
