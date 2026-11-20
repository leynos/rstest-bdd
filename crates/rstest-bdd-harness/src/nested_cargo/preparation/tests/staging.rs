//! A working directory the preparation creates, staged before it is identified.
//!
//! The cases drive a real probe against a compiler shim rather than an injected
//! identity, because the defect is in *where and when* the probe runs: an
//! injected identity is the same text whatever directory exists. The shim is a
//! shell script, so the cases are Unix-only; the ordering they pin is not.
#![cfg(unix)]

use std::{fs, io, os::unix::fs::PermissionsExt as _, path::Path};

use super::populate_profile;
use crate::nested_cargo::{
    Directories,
    Preparation,
    PreparationOutcome,
    PreparationSummary,
    ProfileDestination,
    Sources,
    ensure_prepared_building,
    ensure_prepared_building_staged,
    env_from_vars,
    prepare_now,
    prepare_now_staged,
};

/// A compiler that reports the pin in the directory it is started from.
///
/// It stands in for a `rustup` shim, which selects its toolchain from the
/// directory it runs in. Reading only that directory, not its ancestors, keeps
/// the case independent of whatever is pinned above the temporary directory.
const SHIM: &str = "#!/bin/sh\npin=unpinned\nif [ -f rust-toolchain ]; then pin=$(cat \
                    rust-toolchain); fi\nprintf 'release: %s\\ncommit-hash: shim\\nhost: shim\\n' \
                    \"$pin\"\n";

/// Write the compiler shim at `path`, creating its directory.
fn write_shim(path: &Path) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, SHIM)?;
    fs::set_permissions(path, fs::Permissions::from_mode(0o755))
}

/// A family below `root` whose nested builds run in `root/scratch`.
///
/// The scratch directory is deliberately not created: creating it is the
/// staging step under test. `compiler` is the `RUSTC` the nested build is
/// given, relative or absolute.
fn family(root: &Path, name: &str, compiler: &Path) -> io::Result<Preparation> {
    let target = root.join("target");
    populate_profile(&target)?;
    let environment = env_from_vars(
        vec![("RUSTC".into(), compiler.as_os_str().to_os_string())],
        Some(&target),
        ProfileDestination::Caller,
    )?;
    Preparation::new(
        Sources::new(name, "v", "hash"),
        Directories::new(root.join("stamps"), root.join("scratch")),
        environment,
    )
}

#[test]
fn a_relative_compiler_beneath_an_unstaged_directory_needs_staging() -> io::Result<()> {
    // `RUSTC=tools/rustc` resolves against the directory the nested build
    // runs in, and that directory arrives with the fixture copy. Identified
    // before the copy, the compiler is a path that does not exist.
    let directory = tempfile::tempdir()?;
    let root = fs::canonicalize(directory.path())?;
    let preparation = family(&root, "relative-compiler", Path::new("tools/rustc"))?;
    let stage = || write_shim(&root.join("scratch/tools/rustc"));

    let error = prepare_now(&preparation, || {
        stage()?;
        Ok(PreparationSummary::default())
    })
    .expect_err("a compiler the callback has yet to copy cannot be identified before it runs");
    assert!(
        error.to_string().contains("rustc -vV"),
        "the cold failure is the probe's, got: {error}"
    );
    assert!(
        !root.join("scratch").exists(),
        "the probe must fail before the callback, which is what leaves the tree unstaged"
    );

    prepare_now_staged(&preparation, stage, || Ok(PreparationSummary::default()))?;
    let outcome = ensure_prepared_building(&preparation, true, || {
        panic!("a staged and prepared family must be reused")
    })?;
    assert_eq!(outcome, PreparationOutcome::Reused);
    Ok(())
}

#[test]
fn a_pin_the_preparation_copies_in_is_identified_only_when_staged() -> io::Result<()> {
    let directory = tempfile::tempdir()?;
    let root = fs::canonicalize(directory.path())?;
    let compiler = root.join("toolchain/rustc");
    write_shim(&compiler)?;
    let pin = |family_root: &Path| -> io::Result<()> {
        fs::create_dir_all(family_root.join("scratch"))?;
        fs::write(family_root.join("scratch/rust-toolchain"), "pinned")
    };

    // Unstaged, the callback introduces the pin after the identity was taken
    // from the ancestor, so the stamp names the unpinned compiler and the
    // warm gate, started in the pinned directory, refuses that stamp.
    let unstaged_root = root.join("unstaged");
    let unstaged = family(&unstaged_root, "unstaged-pin", &compiler)?;
    prepare_now(&unstaged, || {
        pin(&unstaged_root)?;
        Ok(PreparationSummary::default())
    })?;
    ensure_prepared_building(&unstaged, true, || {
        panic!("a timed test must never prepare")
    })
    .expect_err("the control must show the stamp disagreeing with the pinned directory");

    // Staged, the pin is in place before either identity is taken, so the
    // cold stamp and the warm gate name the same compiler.
    let staged_root = root.join("staged");
    let staged = family(&staged_root, "staged-pin", &compiler)?;
    prepare_now_staged(
        &staged,
        || pin(&staged_root),
        || Ok(PreparationSummary::default()),
    )?;
    let outcome = ensure_prepared_building(&staged, true, || {
        panic!("a staged and prepared family must be reused")
    })?;
    assert_eq!(outcome, PreparationOutcome::Reused);
    Ok(())
}

#[test]
fn the_staged_gate_stages_only_where_it_may_build() -> io::Result<()> {
    let directory = tempfile::tempdir()?;
    let root = fs::canonicalize(directory.path())?;
    let compiler = root.join("toolchain/rustc");
    write_shim(&compiler)?;
    let preparation = family(&root, "staged-gate", &compiler)?;

    // Under nextest the gate refuses an unprepared family and writes nothing.
    let mut staged = false;
    ensure_prepared_building_staged(
        &preparation,
        true,
        || {
            staged = true;
            Ok(())
        },
        || panic!("a timed test must never prepare"),
    )
    .expect_err("an unprepared family is refused under nextest");
    assert!(!staged, "a gate that may not build must not stage either");

    // Under `cargo test` it stages first, then prepares against that tree.
    let outcome = ensure_prepared_building_staged(
        &preparation,
        false,
        || fs::create_dir_all(root.join("scratch")),
        || Ok(PreparationSummary::default()),
    )?;
    assert_eq!(outcome, PreparationOutcome::Prepared);
    assert!(root.join("scratch").is_dir(), "the lazy path must stage");
    Ok(())
}
