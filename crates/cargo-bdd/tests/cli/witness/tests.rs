//! The witness's own regression cases.
//!
//! A check that only ever runs against a warm tree cannot distinguish a
//! real preparation from a no-op, so each rejection is exercised against
//! output that compiled nothing, and one case is exercised against the
//! build-directory layout that moved the units.

use std::{fs, io, path::Path};

use rstest::{fixture, rstest};

use super::{FIXTURE_TEST_BINARIES, fixture_binaries_present};

/// An empty `debug/deps` directory beneath a retained temporary directory.
///
/// Returned with the `TempDir` because it borrows from it: the directory
/// must outlive every case that writes into it.
///
/// Fallible so a case reports why its fixture could not be built rather
/// than panicking inside shared setup.
#[fixture]
fn deps_directory() -> io::Result<(tempfile::TempDir, std::path::PathBuf)> {
    let scratch = tempfile::tempdir()?;
    let deps = scratch.path().join("debug").join("deps");
    fs::create_dir_all(&deps)?;
    Ok((scratch, deps))
}

/// Write the file names Cargo would leave for one unit.
///
/// Both halves are written, because a real tree carries the dep-info record
/// and the output it describes. The hash suffix is what Cargo adds, so the
/// names here are the shape the witness has to read.
fn write_unit(deps: &Path, stem: &str, hash: &str, output: Option<&str>) -> io::Result<()> {
    fs::write(deps.join(format!("{stem}-{hash}.d")), b"dep-info")?;
    if let Some(output) = output {
        fs::write(deps.join(output), b"unit")?;
    }
    Ok(())
}

#[test]
fn a_tree_of_unrelated_artefacts_is_not_a_prepared_fixture() {
    // The no-op case the preparation test must reject. Every directory the
    // stamp's structural layer looks for is present and populated, so a
    // check that answered "the profile is not empty" would accept it; only a
    // witness naming the fixture's own units can tell the two apart.
    let scratch = tempfile::tempdir().expect("temp dir");
    let deps = scratch.path().join("debug").join("deps");
    fs::create_dir_all(&deps).expect("deps dir");
    fs::create_dir_all(scratch.path().join("debug").join(".fingerprint")).expect("fingerprint dir");
    fs::write(
        scratch
            .path()
            .join("debug")
            .join(".fingerprint")
            .join("unit"),
        b"record",
    )
    .expect("fingerprint record");
    write_unit(
        &deps,
        "libunrelated",
        "0123456789abcdef",
        Some("libunrelated-0123456789abcdef.rlib"),
    )
    .expect("the unrelated unit should be written");
    // Not even one of the fixture's, let alone both.
    write_unit(
        &deps,
        FIXTURE_TEST_BINARIES[0],
        "0123456789abcdef",
        Some("diagnostics_fixture-0123456789abcdef"),
    )
    .expect("the lone fixture unit should be written");

    let present = fixture_binaries_present(&deps).expect("the fixture deps should list");
    assert!(
        !present,
        "a tree holding only one of the fixture's binaries must not be reported as prepared"
    );
}

#[test]
fn the_fixture_binaries_are_recognized_when_present() {
    // The positive control. Without it the test above would pass for a
    // witness that always answered "no". The output is named the way Cargo
    // names a test binary — `<stem>-<hash>` — rather than as a bare stem,
    // because the bare stem is the shape the witness must *not* accept: a
    // dep-info record alone would otherwise witness a unit with no binary.
    let scratch = tempfile::tempdir().expect("temp dir");
    let deps = scratch.path().join("debug").join("deps");
    fs::create_dir_all(&deps).expect("deps dir");
    for stem in FIXTURE_TEST_BINARIES {
        let binary = format!("{stem}-0123456789abcdef");
        write_unit(&deps, stem, "0123456789abcdef", Some(&binary))
            .expect("the fixture unit should be written");
    }

    let present = fixture_binaries_present(&deps).expect("the fixture deps should list");
    assert!(
        present,
        "the witness must recognize both fixture binaries in {}",
        deps.display()
    );
}

#[test]
fn a_dep_info_record_alone_does_not_witness_a_binary() {
    // The narrowest no-op: the record is there, the binary it describes is
    // not. Reading the record as the witness would accept a tree Cargo
    // itself would recompile, which is the whole state the preparation
    // exists to exclude.
    let scratch = tempfile::tempdir().expect("temp dir");
    let deps = scratch.path().join("debug").join("deps");
    fs::create_dir_all(&deps).expect("deps dir");
    for stem in FIXTURE_TEST_BINARIES {
        write_unit(&deps, stem, "0123456789abcdef", None)
            .expect("the dep-info record should be written");
    }

    let present = fixture_binaries_present(&deps).expect("the fixture deps should list");
    assert!(
        !present,
        "a dep-info record without its binary must not be reported as prepared"
    );
}

#[test]
fn a_directory_named_like_a_binary_does_not_witness_one() {
    // The name is the only thing a directory of this shape shares with a
    // test binary, and the walk visits directories as well as files, so a
    // witness that matched on name alone would accept a tree that holds no
    // executable at all. Both stems are given a directory, so the failure
    // cannot come from one of them merely being present.
    let scratch = tempfile::tempdir().expect("temp dir");
    let deps = scratch.path().join("debug").join("deps");
    fs::create_dir_all(&deps).expect("deps dir");
    for stem in FIXTURE_TEST_BINARIES {
        let impostor = deps.join(format!("{stem}-0123456789abcdef"));
        fs::create_dir_all(&impostor).expect("impostor dir");
        fs::write(impostor.join("inside"), b"not a binary").expect("contents");
    }

    let present = fixture_binaries_present(&deps).expect("the fixture deps should list");
    assert!(!present, "a directory named like a test binary is not one");
}

/// What a unit's output is named, and whether that name is an executable.
///
/// The extension policy in one table, because that is what it is: a rule
/// about suffixes, not five unrelated scenarios. `suffix` is appended to
/// `<stem>-<hash>` and `None` names the extensionless Linux form. Each row
/// states the reason it belongs, so a future case has an obvious home
/// rather than becoming a sixth near-copy of the setup.
#[rstest]
#[case::no_extension(None, true, "an extensionless Linux executable is the binary itself")]
#[case::uppercase_exe(
    Some(".EXE"),
    true,
    "Windows names every test binary `.exe`; the filesystem that produced it is case-insensitive, \
     so `.EXE` must witness too"
)]
#[case::pdb(
    Some(".pdb"),
    false,
    "MSVC writes a `.pdb` beside the executable; it carries the full stem, so an extension-blind \
     check accepts a tree whose executable was deleted"
)]
#[case::dwp(
    Some(".dwp"),
    false,
    "a split-dwarf Linux build writes `.dwp` beside the executable, with the same stem"
)]
#[case::dep_info(
    Some(".d"),
    false,
    "the dep-info record is the no-op's own tell: reading it as the witness is the state the \
     preparation exists to exclude"
)]
#[case::rmeta(
    Some(".rmeta"),
    false,
    "`cargo check` leaves a `.rmeta` where `cargo build` would leave the linked output, which is \
     a warm check graph and a cold build one"
)]
fn only_an_executable_witnesses_a_test_binary(
    deps_directory: io::Result<(tempfile::TempDir, std::path::PathBuf)>,
    #[case] suffix: Option<&str>,
    #[case] witnesses: bool,
    #[case] reason: &str,
) {
    // The positive rows are what keep the negative ones from passing for a
    // witness that always answers "no", and the negative rows are what keep
    // the positive ones from passing for a check that matches any name with
    // the right stem.
    let (_scratch, deps) = deps_directory.expect("the deps fixture should be built");
    let suffix = suffix.unwrap_or_default();
    for stem in FIXTURE_TEST_BINARIES {
        fs::write(
            deps.join(format!("{stem}-0123456789abcdef{suffix}")),
            b"contents",
        )
        .expect("the output should be written");
    }

    let present = fixture_binaries_present(&deps).expect("the fixture deps should list");
    assert_eq!(present, witnesses, "{reason}");
}

#[test]
fn a_missing_profile_is_not_a_prepared_fixture() {
    // A target that was never built at all, rather than one built by
    // something else. The witness reports the unreadable directory as an
    // error: a tree whose profile cannot be listed is not one that has been
    // prepared, and reporting it as an empty set would lose that.
    let scratch = tempfile::tempdir().expect("temp dir");

    let error = fixture_binaries_present(&scratch.path().join("debug"))
        .expect_err("an absent profile must not be reported as prepared");
    assert_eq!(error.kind(), io::ErrorKind::NotFound, "{error}");
}

#[test]
fn the_witness_finds_binaries_under_the_build_dir_layout() {
    // Cargo 1.100.0 moved every unit out of `<profile>/deps` into
    // `<profile>/build/<pkg>/<hash>/out` and left `deps` absent. The
    // witness named `deps`, so on Windows CI it read a directory that no
    // longer existed and every preparation of this family failed. The walk
    // must find the binaries wherever Cargo put them.
    let scratch = tempfile::tempdir().expect("temp dir");
    let out = scratch
        .path()
        .join("debug")
        .join("build/cargo-bdd/0123456789abcdef/out");
    fs::create_dir_all(&out).expect("unit dir");
    for stem in FIXTURE_TEST_BINARIES {
        write_unit(
            &out,
            stem,
            "0123456789abcdef",
            Some(&format!("{stem}-0123456789abcdef")),
        )
        .expect("the unit-directory fixture should be written");
    }
    assert!(
        !scratch.path().join("debug").join("deps").exists(),
        "the fixture must model the layout that has no `deps` at all"
    );

    let present =
        fixture_binaries_present(&scratch.path().join("debug")).expect("the profile should list");
    assert!(
        present,
        "the witness must find the binaries below the profile, not only in `deps`"
    );
}
