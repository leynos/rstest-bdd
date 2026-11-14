//! Unit tests for the discovery walk.
//!
//! The two layouts are the whole point of this module: one release moved every
//! compiled unit from `<profile>/deps` to `<profile>/build/<pkg>/<hash>/out` and
//! left `deps` absent, which is what a check that named the directory could not
//! survive. Both shapes are exercised here, and each structural requirement is
//! shown to be individually necessary.

use std::fs;

use super::{discover, entries_of, is_output_of, outputs_in};

/// Write `units` into `directory` as rustc leaves them: a `.d` record and its
/// output side by side.
fn write_units(directory: &std::path::Path, units: &[(&str, &str)]) {
    fs::create_dir_all(directory).expect("unit dir");
    for (stem, extension) in units {
        fs::write(directory.join(format!("{stem}.d")), b"dep-info").expect("record");
        fs::write(directory.join(format!("lib{stem}.{extension}")), b"unit").expect("output");
    }
}

/// The classic shape: `<profile>/deps` for units, `<profile>/.fingerprint` for
/// the store.
#[test]
fn the_classic_deps_layout_is_discovered() {
    let directory = tempfile::tempdir().expect("temp dir");
    let profile = directory.path().join("debug");
    write_units(&profile.join("deps"), &[("unit", "rlib")]);
    fs::create_dir_all(profile.join(".fingerprint")).expect("store");
    fs::write(profile.join(".fingerprint/lib-unit"), b"record").expect("record");

    let layout = discover(&profile).expect("discover");
    assert_eq!(
        layout.units,
        vec![profile.join("deps")],
        "the directory holding the record is the unit directory"
    );
    assert_eq!(
        layout.fingerprints,
        vec![profile.join(".fingerprint")],
        "the profile-level store must be found"
    );
    assert!(layout.is_usable());
}

/// Cargo 1.100.0's shape: units and their store in one
/// `<profile>/build/<pkg>/<hash>` directory, with `deps` absent entirely.
///
/// This is the layout that broke the previous check, so it is the case that
/// must not regress.
#[test]
fn the_build_dir_out_layout_is_discovered() {
    let directory = tempfile::tempdir().expect("temp dir");
    let profile = directory.path().join("debug");
    let hash = profile.join("build/unit/0123456789abcdef");
    write_units(&hash.join("out"), &[("unit", "rlib")]);
    fs::create_dir_all(hash.join("fingerprint")).expect("store");
    fs::write(hash.join("fingerprint/lib-unit"), b"record").expect("record");

    assert!(
        !profile.join("deps").exists(),
        "the fixture must model the layout that has no `deps` at all"
    );
    let layout = discover(&profile).expect("discover");
    assert_eq!(
        layout.units,
        vec![hash.join("out")],
        "the unit directory is the `out` directory the records were written to"
    );
    assert_eq!(
        layout.fingerprints,
        vec![hash.join("fingerprint")],
        "the per-unit store must be found"
    );
    assert!(layout.is_usable());
}

/// A profile holding units but no fingerprint store is not usable.
#[test]
fn units_without_a_store_are_not_usable() {
    let directory = tempfile::tempdir().expect("temp dir");
    let profile = directory.path().join("debug");
    write_units(&profile.join("deps"), &[("unit", "rlib")]);

    let layout = discover(&profile).expect("discover");
    assert!(!layout.units.is_empty(), "the units are there");
    assert!(
        layout.fingerprints.is_empty(),
        "no store was written, so none may be reported"
    );
    assert!(
        !layout.is_usable(),
        "Cargo rebuilds a tree with no record that its units are current"
    );
}

/// A profile holding a store but no units is not usable.
#[test]
fn a_store_without_units_is_not_usable() {
    let directory = tempfile::tempdir().expect("temp dir");
    let profile = directory.path().join("debug");
    fs::create_dir_all(profile.join(".fingerprint")).expect("store");
    fs::write(profile.join(".fingerprint/lib-unit"), b"record").expect("record");

    let layout = discover(&profile).expect("discover");
    assert!(layout.units.is_empty(), "no unit directory was written");
    assert!(!layout.fingerprints.is_empty(), "the store is there");
    assert!(
        !layout.is_usable(),
        "there is nothing for a record to speak about"
    );
}

/// An empty fingerprint store does not count.
#[test]
fn an_empty_store_is_not_a_store() {
    let directory = tempfile::tempdir().expect("temp dir");
    let profile = directory.path().join("debug");
    write_units(&profile.join("deps"), &[("unit", "rlib")]);
    fs::create_dir_all(profile.join(".fingerprint")).expect("store");

    let layout = discover(&profile).expect("discover");
    assert!(
        layout.fingerprints.is_empty(),
        "an emptied store holds no record Cargo can read"
    );
    assert!(!layout.is_usable());
}

/// A profile that does not exist is an absent layout, not an error.
#[test]
fn an_absent_profile_is_not_an_error() {
    let directory = tempfile::tempdir().expect("temp dir");
    let layout = discover(&directory.path().join("debug")).expect("absence is not a fault");
    assert_eq!(layout.units, Vec::<std::path::PathBuf>::new());
    assert_eq!(layout.fingerprints, Vec::<std::path::PathBuf>::new());
    assert!(!layout.is_usable());
}

/// The incremental cache is pruned, so its size cannot decide the walk.
#[test]
fn the_incremental_cache_is_not_searched() {
    let directory = tempfile::tempdir().expect("temp dir");
    let profile = directory.path().join("debug");
    write_units(&profile.join("deps"), &[("unit", "rlib")]);
    fs::create_dir_all(profile.join(".fingerprint")).expect("store");
    fs::write(profile.join(".fingerprint/lib-unit"), b"record").expect("record");
    // A record inside the incremental cache would make the cache look like a
    // unit directory if it were searched. It holds scratch state Cargo
    // regenerates, so it must not be.
    write_units(&profile.join("incremental/unit"), &[("scratch", "rlib")]);

    let layout = discover(&profile).expect("discover");
    assert_eq!(
        layout.units,
        vec![profile.join("deps")],
        "the incremental cache must not be reported as a unit directory"
    );
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

/// A directory's outputs are the same-stem entries beside its records.
#[test]
fn outputs_pair_with_their_records() {
    let directory = tempfile::tempdir().expect("temp dir");
    let units = directory.path();
    write_units(units, &[("linked", "rlib"), ("checked", "rmeta")]);
    // A unit that emitted both forms, and an unrelated artefact that must not
    // be attributed to either.
    fs::write(units.join("liblinked.rmeta"), b"unit").expect("metadata");
    fs::write(units.join("libunrelated.rlib"), b"unit").expect("unrelated");

    let outputs = outputs_in(units).expect("outputs");
    assert_eq!(
        outputs,
        vec![
            "libchecked.rmeta".to_owned(),
            "liblinked.rlib".to_owned(),
            "liblinked.rmeta".to_owned(),
        ],
        "only the forms of units with a record may be paired: {outputs:?}"
    );
}

/// The listing is sorted, so a record built from it is stable.
#[test]
fn entries_are_listed_in_a_stable_order() {
    let directory = tempfile::tempdir().expect("temp dir");
    for name in ["zz.d", "aa.d", "mm.d"] {
        fs::write(directory.path().join(name), b"record").expect("write");
    }
    assert_eq!(
        entries_of(directory.path()).expect("entries"),
        vec!["aa.d".to_owned(), "mm.d".to_owned(), "zz.d".to_owned()]
    );
}

/// A directory that cannot be listed reports the fault.
#[test]
fn an_unlistable_directory_reports_the_error() {
    let directory = tempfile::tempdir().expect("temp dir");
    let error = entries_of(&directory.path().join("absent"))
        .expect_err("a missing directory is a read fault here");
    assert_eq!(error.kind(), std::io::ErrorKind::NotFound);
}
