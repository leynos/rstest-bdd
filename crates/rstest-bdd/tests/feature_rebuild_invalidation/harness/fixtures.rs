//! Scratch-copy, stamp-protocol and manifest-rewrite helpers for the
//! nested-cargo harness.
//!
//! The regression tests never mutate the checked-in fixture: they copy it to
//! `target/tests/rebuild-invalidation/fixture` under the shared workspace
//! `target/`, rewrite its path dependencies to absolute paths, and mutate the
//! copy. Restoring only the feature file is not sufficient — a scratch
//! directory left over from an older fixture version would keep a stale
//! `tests/invalidation.rs`, `Cargo.toml` and `Cargo.lock`, and the test would
//! either fail inexplicably or pass against the wrong sources. A
//! timestamp-free stamp protocol guards against that: the scratch is rebuilt
//! wholesale whenever the stamp (a hash of the source tree, keyed by this
//! harness's protocol version) differs from the recorded one or the manifest
//! still carries relative dependency paths.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// Path of the workspace's shared build-output directory.
pub(crate) fn shared_target_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target")
}

/// Root scratch directory owned exclusively by this harness.
pub(crate) fn scratch_root() -> PathBuf {
    shared_target_dir().join("tests/rebuild-invalidation")
}

/// The copied fixture crate inside the scratch area.
pub(crate) fn scratch_fixture_dir() -> PathBuf {
    scratch_root().join("fixture")
}

/// The checked-in fixture crate source.
pub(crate) fn source_fixture_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/rebuild_invalidation")
}

/// The fixture's feature file path inside the scratch area.
pub(crate) fn scratch_feature_file() -> PathBuf {
    scratch_fixture_dir().join("tests/features/invalidation.feature")
}

/// Immediate children of a directory, for recursive copying.
fn copy_dir_recursive(from: &Path, to: &Path) -> io::Result<()> {
    fs::create_dir_all(to)?;
    for entry in fs::read_dir(from)? {
        let entry = entry?;
        let src = entry.path();
        let dst = to.join(entry.file_name());
        if src.is_dir() {
            copy_dir_recursive(&src, &dst)?;
        } else {
            fs::copy(&src, &dst)?;
        }
    }
    Ok(())
}

/// Collect the source fixture tree as sorted (relative path, contents) pairs,
/// excluding the lockfile — Cargo regenerates the scratch lockfile, so it must
/// not invalidate the source stamp.
fn collect_source_entries() -> Vec<(PathBuf, Vec<u8>)> {
    fn walk(dir: &Path, base: &Path, out: &mut Vec<(PathBuf, Vec<u8>)>) -> io::Result<()> {
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let rel = base.join(entry.file_name());
            if entry.file_type()?.is_dir() {
                walk(&entry.path(), &rel, out)?;
            } else if entry.file_name() != "Cargo.lock" {
                let contents = fs::read(entry.path())?;
                out.push((rel, contents));
            }
        }
        Ok(())
    }

    let mut entries = Vec::new();
    if let Err(err) = walk(&source_fixture_dir(), Path::new(""), &mut entries) {
        panic!("cannot hash the fixture tree: {err}");
    }
    entries
}

/// A stable hash of the checked-in fixture tree, keying the stamp protocol.
fn source_tree_hash() -> String {
    use std::hash::{Hash, Hasher};

    let mut entries = collect_source_entries();
    entries.sort();
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    for (rel, contents) in &entries {
        rel.hash(&mut hasher);
        contents.hash(&mut hasher);
    }
    format!("{:016x}", hasher.finish())
}

/// Protocol version for the scratch stamp. Bump when the scratch layout or
/// the rewrite rules change, so stale scratch trees from an older protocol
/// (with their stale manifests) are always re-created instead of reused.
const SCRATCH_PROTOCOL_VERSION: &str = "2";

/// Copy the checked-in fixture into the scratch area unless the stamp says
/// the copy is already current, then rewrite its path dependencies to
/// absolute paths.
///
/// The stamp is written **last**, so a copy interrupted halfway can never be
/// mistaken for a complete one; a scratch tree left over from an older
/// fixture version (stale `tests/invalidation.rs`, `Cargo.toml`, `Cargo.lock`)
/// would otherwise fail the test inexplicably or pass against the wrong
/// sources. The version prefix additionally forces a re-copy whenever this
/// harness's own rewrite rules change (a leave-over from the pre-versioned
/// protocol was the failure mode that motivated it).
pub(crate) fn ensure_fixture_copied() {
    let stamp_path = scratch_root().join(".stamp");
    let stamp = format!("{SCRATCH_PROTOCOL_VERSION}:{}", source_tree_hash());
    if !scratch_fixture_dir().is_dir() {
        recopy_fixture(&stamp_path, &stamp);
        return;
    }
    if read_stamp(&stamp_path) != stamp || !manifest_path_deps_are_absolute() {
        recopy_fixture(&stamp_path, &stamp);
    }
}

/// Replace the scratch copy wholesale and record the current stamp.
fn recopy_fixture(stamp_path: &Path, stamp: &str) {
    if scratch_fixture_dir().exists() {
        if let Err(err) = fs::remove_dir_all(scratch_fixture_dir()) {
            panic!("remove stale scratch fixture: {err}");
        }
    }
    if let Err(err) = copy_dir_recursive(&source_fixture_dir(), &scratch_fixture_dir()) {
        panic!("copy fixture to scratch: {err}");
    }
    rewrite_manifest_path_deps();
    if let Err(err) = fs::write(stamp_path, stamp) {
        panic!("write stamp file: {err}");
    }
}

/// The recorded stamp, or the empty string when the scratch is uninitialised.
fn read_stamp(path: &Path) -> String {
    fs::read_to_string(path).unwrap_or_default()
}

/// Whether every `path = "…"` value in the scratch manifest is absolute.
///
/// Guards against a stale scratch whose manifest was written by an older
/// (possibly buggy) rewrite round while the source tree — and therefore the
/// stamp — is unchanged.
fn manifest_path_deps_are_absolute() -> bool {
    let Ok(text) = fs::read_to_string(scratch_fixture_dir().join("Cargo.toml")) else {
        return false;
    };
    text.lines().all(|line| {
        let Some((_, after_marker)) = line.split_once("path = \"") else {
            return true;
        };
        match after_marker.split_once('"') {
            Some((value, _)) => Path::new(value).is_absolute(),
            None => true,
        }
    })
}

/// Rewrite the scratch manifest's relative path dependencies to absolute
/// paths, then assert no `..` remains in any dependency path.
///
/// The fixture lives at `crates/rstest-bdd/tests/fixtures/rebuild_invalidation`
/// but the scratch copy moves it to `target/tests/rebuild-invalidation/fixture`
/// — a change of directory depth, so the relative `path = "../../../../../…"`
/// values overshoot the workspace root when resolved from the *scratch*
/// location. Each value is therefore resolved against the **source** fixture
/// directory (whose depth the `..` count matches) and the resolved absolute
/// target is written into the scratch manifest. The scratch manifest is not
/// checked in, so absolute paths there breach nothing: Constraint 2 of the
/// execplan governs the macro's emitted tokens, not scratch build inputs.
fn rewrite_manifest_path_deps() {
    let manifest_path = scratch_fixture_dir().join("Cargo.toml");
    let text = match fs::read_to_string(&manifest_path) {
        Ok(text) => text,
        Err(err) => panic!(
            "cannot read the scratch manifest {}: {err}",
            manifest_path.display()
        ),
    };
    let mut rewritten_lines = Vec::new();
    let mut saw_relative = false;
    for line in text.lines() {
        let Some((before, after_marker)) = line.split_once("path = \"") else {
            rewritten_lines.push(line.to_owned());
            continue;
        };
        let Some((value, tail)) = after_marker.split_once('"') else {
            rewritten_lines.push(line.to_owned());
            continue;
        };
        if Path::new(value).is_absolute() {
            rewritten_lines.push(line.to_owned());
            continue;
        }
        saw_relative = true;
        // Resolve against the source fixture directory, not the scratch copy:
        // the `..` counts in the checked-in manifest are relative to the
        // source location. `split_once` drops the delimiters it splits on, so
        // both quotes around the value are written back explicitly.
        let resolved = normalize_lexically(&source_fixture_dir().join(value));
        rewritten_lines.push(format!("{before}path = \"{}\"{tail}", resolved.display()));
    }
    assert!(
        saw_relative,
        "expected relative path dependencies in the scratch manifest"
    );
    let rewritten = rewritten_lines.join("\n");
    assert!(
        !rewritten.contains("\".."),
        "rewritten manifest still contains a relative dependency path:\n{rewritten}"
    );
    if let Err(err) = fs::write(&manifest_path, rewritten) {
        panic!("cannot write the scratch manifest: {err}");
    }
}

/// Lexically normalize `..` segments (no filesystem access).
fn normalize_lexically(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for component in path.components() {
        match component {
            std::path::Component::ParentDir => {
                out.pop();
            }
            std::path::Component::CurDir => {}
            other => out.push(other.as_os_str()),
        }
    }
    out
}

/// Restore the scratch feature file from the checked-in source.
///
/// The rebuild experiment deliberately edits the scratch copy, and the stamp
/// cannot tell the edited scratch from a pristine one (the source is
/// unchanged). Every run therefore resets the feature file before replaying
/// the experiment, so a previous run's edit can never make the current run
/// think the file "already" contains the new expectation.
pub(crate) fn restore_feature_file() {
    let src = source_fixture_dir().join("tests/features/invalidation.feature");
    let dst = scratch_feature_file();
    if let Err(err) = fs::copy(&src, &dst) {
        panic!(
            "cannot restore scratch feature file {} from {}: {err}",
            dst.display(),
            src.display()
        );
    }
}

/// Normalize a path for stable dep-info comparison: `/` separators, folded
/// case (Windows filesystems are case-insensitive; rustc writes paths as
/// given).
pub(crate) fn normalize_dep_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/").to_lowercase()
}
