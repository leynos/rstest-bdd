//! Unit tests for the recorded-name check and its structural layer.
//!
//! Exercises [`record`], [`all_present`] and the verification entry points of
//! the parent [`artefacts`](super) module: what a preparation records for the
//! units a build left behind, how the gate compares that record against the
//! tree it finds, and which removals it refuses. The parent module is the
//! layer between a stamp and the filesystem, so these cases supply a profile
//! directly rather than going through `lifecycle`; the end-to-end eviction
//! cases, which need a written stamp, live in that module's own tests.

use std::fs;

use super::{all_present, record};
use crate::nested_cargo::{Preparation, ProfileDestination, Sources, env_from_vars};

/// The profile directory a `preparation_with` fixture vouches for.
///
/// The fixture always creates exactly one, so a missing one is a broken
/// fixture rather than a case under test.
pub(super) fn profile_of(preparation: &Preparation) -> std::io::Result<std::path::PathBuf> {
    preparation
        .artefacts
        .first()
        .cloned()
        .ok_or_else(|| std::io::Error::other("the fixture describes no profile"))
}

/// Write units into `directory` as rustc leaves them: a `.d` record and its
/// output side by side.
pub(super) fn write_units(
    directory: &std::path::Path,
    units: &[(&str, &str)],
) -> std::io::Result<()> {
    fs::create_dir_all(directory)?;
    for (stem, extension) in units {
        fs::write(directory.join(format!("{stem}.d")), b"dep-info")?;
        fs::write(directory.join(format!("lib{stem}.{extension}")), b"unit")?;
    }
    Ok(())
}

/// A profile holding `units` in `unit_dir` and a store at `store`, both named
/// relative to `<profile>`.
///
/// The two fixtures below describe the same thing — a profile holding a unit
/// directory and a store beside it — and differ only in where Cargo would have
/// put them, so they share this scaffolding and pass the layout in.
fn preparation_in(
    unit_dir: &str,
    store: &str,
    units: &[(&str, &str)],
) -> std::io::Result<(tempfile::TempDir, Preparation)> {
    let directory = tempfile::tempdir()?;
    let target = directory.path().join("target");
    let profile = target.join("debug");
    write_units(&profile.join(unit_dir), units)?;
    write_store(&profile.join(store), "unit")?;
    let preparation = preparation_for(directory.path(), &target)?;
    Ok((directory, preparation))
}

/// Write a fingerprint store holding one unit's record directory.
///
/// Cargo files a unit's records under `<store>/<unit>/`, so a fixture that put
/// them loose in the store would model a layout Cargo never writes and would
/// leave the record layer untested. The unit name carries the hash the compiled
/// unit carries, because that is what pairs the two.
pub(super) fn write_store(store: &std::path::Path, unit: &str) -> std::io::Result<()> {
    let records = store.join(format!("{unit}-0123456789abcdef"));
    fs::create_dir_all(&records)?;
    fs::write(records.join(format!("lib-{unit}")), b"record")?;
    fs::write(records.join(format!("lib-{unit}.json")), b"{}")?;
    Ok(())
}

/// A profile in the classic `<profile>/deps` + `<profile>/.fingerprint` shape.
pub(super) fn preparation_with(
    units: &[(&str, &str)],
) -> std::io::Result<(tempfile::TempDir, Preparation)> {
    preparation_in("deps", ".fingerprint", units)
}

/// A profile in Cargo 1.100.0's `<profile>/build/<pkg>/<hash>/out` shape.
fn preparation_with_new_layout(
    units: &[(&str, &str)],
) -> std::io::Result<(tempfile::TempDir, Preparation)> {
    preparation_in(
        "build/unit/0123456789abcdef/out",
        "build/unit/0123456789abcdef/fingerprint",
        units,
    )
}

/// Describe `target/debug` as the artefacts a preparation rooted at `root`
/// vouches for.
fn preparation_for(
    root: &std::path::Path,
    target: &std::path::Path,
) -> std::io::Result<Preparation> {
    let environment = env_from_vars(Vec::new(), Some(target), ProfileDestination::Caller)?;
    Preparation::new(
        Sources::new("unit", "v", "hash"),
        root.to_path_buf(),
        environment,
    )
}

/// Both Cargo layouts are discovered and accepted.
///
/// This is the regression for the release that moved every unit: 1.100.0
/// relocated `<profile>/deps` to `<profile>/build/<pkg>/<hash>/out` and kept
/// nothing in `deps`, which a check that named the directory refused outright —
/// on Windows CI it failed every preparation of the CLI family. Discovery must
/// accept both shapes, because the workspace supports both toolchains.
#[test]
fn both_cargo_layouts_are_accepted() -> std::io::Result<()> {
    for (name, build) in [
        (
            "classic deps",
            preparation_with
                as fn(&[(&str, &str)]) -> std::io::Result<(tempfile::TempDir, Preparation)>,
        ),
        ("build-dir out", preparation_with_new_layout),
    ] {
        let (_directory, preparation) = build(&[("unit", "rlib")])?;
        assert!(
            all_present(&preparation, None).expect("structural check"),
            "{name} must be a usable prepared profile"
        );
        let recorded = record(&preparation).expect("record");
        let names = recorded.first().expect("the fixture describes one profile");
        // Two kinds of name, and both are needed: the output rustc left, and
        // the fingerprint record Cargo reads to decide that output is current.
        // The output is qualified by the directory holding the unit, because
        // two units in different directories can share a stem.
        let outputs: Vec<&String> = names
            .iter()
            .filter(|recorded| {
                std::path::Path::new(recorded)
                    .extension()
                    .is_some_and(|extension| extension.eq_ignore_ascii_case("rlib"))
            })
            .collect();
        assert_eq!(
            outputs.len(),
            1,
            "{name} must record exactly one output: {names:?}"
        );
        assert!(
            outputs
                .first()
                .is_some_and(|only| only.ends_with("libunit.rlib")),
            "{name} must record the output rustc left: {names:?}"
        );
        assert!(
            names.iter().any(|recorded| recorded.ends_with("/lib-unit")),
            "{name} must record the unit's fingerprint record: {names:?}"
        );
    }
    Ok(())
}

/// The same unit in two different directories is recorded twice.
///
/// Two units can share a stem — the same crate compiled for a build script and
/// for the library — and the record must not let one witness the other. The
/// directory is part of the recorded name for that reason.
#[test]
fn units_in_different_directories_are_recorded_separately() -> std::io::Result<()> {
    let (_directory, preparation) = preparation_with_new_layout(&[("unit", "rlib")])?;
    let profile = profile_of(&preparation)?;
    write_units(
        &profile.join("build/other/fedcba9876543210/out"),
        &[("unit", "rlib")],
    )?;

    let recorded = record(&preparation).expect("record");
    let names = recorded.first().expect("the fixture describes one profile");
    // The two outputs, in order; the fingerprint records are asserted
    // separately because they are the other half of what preparation records.
    let outputs: Vec<&str> = names
        .iter()
        .filter(|name| {
            std::path::Path::new(name)
                .extension()
                .is_some_and(|extension| extension.eq_ignore_ascii_case("rlib"))
        })
        .map(String::as_str)
        .collect();
    assert_eq!(
        outputs,
        [
            "build/other/fedcba9876543210/out/libunit.rlib",
            "build/unit/0123456789abcdef/out/libunit.rlib",
        ],
        "each unit's output must be recorded under the directory holding it"
    );
    Ok(())
}

/// A profile that cannot be inspected reports the fault, not an eviction.
#[cfg(unix)]
#[test]
fn an_uninspectable_profile_reports_the_read_error() -> std::io::Result<()> {
    // Folding a metadata fault into an eviction reports a tree that cannot be
    // inspected as one that is known to be gone. The reader cannot act on that,
    // and an unreadable tree is exactly the case where guessing is unsafe.
    // A symlink loop reaches the state without any permission setup, which
    // would not hold when the suite runs as root.
    let (_directory, preparation) = preparation_with(&[("unit", "rlib")])?;
    let profile = profile_of(&preparation)?;
    fs::remove_dir_all(&profile)?;
    std::os::unix::fs::symlink(&profile, &profile)?;

    let error = all_present(&preparation, None)
        .expect_err("a profile that cannot be inspected must not pass as evicted or present");
    assert_ne!(
        error.kind(),
        std::io::ErrorKind::NotFound,
        "a loop is not an absence: {error}"
    );
    Ok(())
}

// The recorded-name cases live in a submodule so this file keeps its own
// share of the repository's line budget.
mod records;
// The shared-profile cases are a third concern again — whose units a stamp may
// speak about — and get a file of their own for the same reason.
mod siblings;
// The replacement cases are a fourth: what the *caller* must exclude from the
// set the predicate is given, which no test of the predicate alone can reach.
mod replacements;
