//! Unit tests for the recorded-name check and its structural layer.

use std::fs;

use super::{Eviction, all_present, evicted, record};
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

/// Write units into `directory` as rustc leaves them: a `.d` record and its
/// output side by side.
fn write_units(directory: &std::path::Path, units: &[(&str, &str)]) {
    fs::create_dir_all(directory).expect("unit dir");
    for (stem, extension) in units {
        fs::write(directory.join(format!("{stem}.d")), b"dep-info").expect("record");
        fs::write(directory.join(format!("lib{stem}.{extension}")), b"unit").expect("output");
    }
}

/// A profile in the classic `<profile>/deps` + `<profile>/.fingerprint` shape.
fn preparation_with(units: &[(&str, &str)]) -> (tempfile::TempDir, Preparation) {
    let directory = tempfile::tempdir().expect("temp dir");
    let target = directory.path().join("target");
    let profile = target.join("debug");
    write_units(&profile.join("deps"), units);
    fs::create_dir_all(profile.join(".fingerprint")).expect("fingerprint dir");
    fs::write(profile.join(".fingerprint/record"), b"record").expect("fingerprint");
    let preparation = preparation_for(directory.path(), &target);
    (directory, preparation)
}

/// A profile in Cargo 1.100.0's `<profile>/build/<pkg>/<hash>/out` shape.
fn preparation_with_new_layout(units: &[(&str, &str)]) -> (tempfile::TempDir, Preparation) {
    let directory = tempfile::tempdir().expect("temp dir");
    let target = directory.path().join("target");
    let profile = target.join("debug");
    let hash = profile.join("build/unit/0123456789abcdef");
    write_units(&hash.join("out"), units);
    fs::create_dir_all(hash.join("fingerprint")).expect("fingerprint dir");
    fs::write(hash.join("fingerprint/lib-unit"), b"record").expect("fingerprint");
    let preparation = preparation_for(directory.path(), &target);
    (directory, preparation)
}

/// Describe `target/debug` as the artefacts a preparation rooted at `root`
/// vouches for.
fn preparation_for(root: &std::path::Path, target: &std::path::Path) -> Preparation {
    let environment = env_from_vars(Vec::new(), Some(target), ProfileDestination::Caller)
        .expect("the fixture environment should resolve");
    Preparation::new(
        Sources::new("unit", "v", "hash"),
        root.to_path_buf(),
        environment,
    )
    .expect("the fixture preparation should describe its artefacts")
}

/// Both Cargo layouts are discovered and accepted.
///
/// This is the regression for the release that moved every unit: 1.100.0
/// relocated `<profile>/deps` to `<profile>/build/<pkg>/<hash>/out` and kept
/// nothing in `deps`, which a check that named the directory refused outright —
/// on Windows CI it failed every preparation of the CLI family. Discovery must
/// accept both shapes, because the workspace supports both toolchains.
#[test]
fn both_cargo_layouts_are_accepted() {
    for (name, build) in [
        (
            "classic deps",
            preparation_with as fn(&[(&str, &str)]) -> (tempfile::TempDir, Preparation),
        ),
        ("build-dir out", preparation_with_new_layout),
    ] {
        let (_directory, preparation) = build(&[("unit", "rlib")]);
        assert!(
            all_present(&preparation, None).expect("structural check"),
            "{name} must be a usable prepared profile"
        );
        let recorded = record(&preparation).expect("record");
        let names = recorded.first().expect("the fixture describes one profile");
        // The recorded name is qualified by the directory holding the unit,
        // because two units in different directories can share a stem.
        assert_eq!(
            names.len(),
            1,
            "{name} must record exactly one output: {names:?}"
        );
        assert!(
            names
                .first()
                .is_some_and(|only| only.ends_with("libunit.rlib")),
            "{name} must record the output rustc left: {names:?}"
        );
    }
}

/// The same unit in two different directories is recorded twice.
///
/// Two units can share a stem — the same crate compiled for a build script and
/// for the library — and the record must not let one witness the other. The
/// directory is part of the recorded name for that reason.
#[test]
fn units_in_different_directories_are_recorded_separately() {
    let (_directory, preparation) = preparation_with_new_layout(&[("unit", "rlib")]);
    let profile = profile_of(&preparation);
    write_units(
        &profile.join("build/other/fedcba9876543210/out"),
        &[("unit", "rlib")],
    );

    let recorded = record(&preparation).expect("record");
    let names = recorded.first().expect("the fixture describes one profile");
    assert_eq!(
        names,
        &vec![
            "build/other/fedcba9876543210/out/libunit.rlib".to_owned(),
            "build/unit/0123456789abcdef/out/libunit.rlib".to_owned(),
        ],
        "each unit's output must be recorded under the directory holding it"
    );
}

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

    let stamp = super::Stamp { recorded };
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

    let stamp = super::Stamp { recorded };
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
        vec![vec!["deps/libchecked.rmeta".to_owned()]],
        "only the forms the preparation produced may be vouched for"
    );
    let stamp = super::Stamp { recorded };
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

/// A profile that cannot be inspected reports the fault, not an eviction.
#[cfg(unix)]
#[test]
fn an_uninspectable_profile_reports_the_read_error() {
    // Folding a metadata fault into an eviction reports a tree that cannot be
    // inspected as one that is known to be gone. The reader cannot act on that,
    // and an unreadable tree is exactly the case where guessing is unsafe.
    // A symlink loop reaches the state without any permission setup, which
    // would not hold when the suite runs as root.
    let (_directory, preparation) = preparation_with(&[("unit", "rlib")]);
    let profile = profile_of(&preparation);
    fs::remove_dir_all(&profile).expect("evict the directory");
    std::os::unix::fs::symlink(&profile, &profile).expect("symlink loop");

    let error = all_present(&preparation, None)
        .expect_err("a profile that cannot be inspected must not pass as evicted or present");
    assert_ne!(
        error.kind(),
        std::io::ErrorKind::NotFound,
        "a loop is not an absence: {error}"
    );
}
