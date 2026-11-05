//! Hashing the fixture inputs a prepared artefact directory derives from.
//!
//! A preparation stamp must be invalidated when the fixture sources that
//! produced the artefacts change, and only then. A nested build writes its own
//! output inside the tree being hashed, so build output and the stamp itself
//! are excluded: otherwise every preparation would invalidate the fingerprint
//! that describes it, and each run would prepare again.

use std::{
    collections::hash_map::DefaultHasher,
    fs,
    hash::{Hash as _, Hasher},
    io,
    path::{Path, PathBuf},
};

use super::stamp::{PREPARED_FILE_SUFFIX, STAMP_VERSION};

/// Directory names that hold build output rather than fixture inputs.
///
/// `target` holds the nested build's artefacts, `wip` holds `trybuild`'s
/// scratch files and `.git` holds version control state. None of them changes
/// what the fixture compiles.
const BUILD_OUTPUT_ENTRIES: [&str; 3] = ["target", "wip", ".git"];

/// Whether an entry name is build output rather than an input.
///
/// The stamp files the preparation module writes are excluded too: they name
/// the fingerprint computed from these inputs, so counting them would make
/// every preparation invalidate the fingerprint that describes it.
fn is_build_output(name: &str) -> bool {
    BUILD_OUTPUT_ENTRIES.contains(&name) || name.ends_with(PREPARED_FILE_SUFFIX)
}

/// Hash the inputs a preparation family's artefacts derive from.
///
/// `roots` may name files or directories; directories are walked in sorted
/// order and every regular file contributes its relative path, length and
/// bytes. Directory entries named `target`, `.git` or `wip` are skipped, as
/// are the stamp files the preparation module writes, so a nested build never
/// invalidates the fingerprint that describes it.
///
/// `wip` is excluded because `trybuild` writes its scratch directory there,
/// beside the crate under test rather than inside the target directory. The
/// preparation computes this hash before it runs the build that creates that
/// directory, so counting it would make every stamp stale the moment it was
/// written.
///
/// # Errors
///
/// Returns the read error when a named input or one of its files cannot be
/// read.
pub fn input_hash(roots: &[PathBuf]) -> io::Result<String> {
    let mut hasher = DefaultHasher::new();
    STAMP_VERSION.hash(&mut hasher);
    for root in roots {
        hash_entry(root, &mut hasher)?;
    }
    Ok(format!("{:016x}", hasher.finish()))
}

/// Feed one file or directory tree into `hasher`.
fn hash_entry(path: &Path, hasher: &mut impl Hasher) -> io::Result<()> {
    let metadata = fs::symlink_metadata(path)?;
    if metadata.is_file() {
        path.file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .hash(hasher);
        let contents = fs::read(path)?;
        contents.hash(hasher);
        return Ok(());
    }
    if !metadata.is_dir() {
        return Ok(());
    }
    let mut entries: Vec<PathBuf> = fs::read_dir(path)?
        .map(|entry| entry.map(|entry| entry.path()))
        .collect::<io::Result<_>>()?;
    entries.sort();
    for entry in entries {
        let name = entry
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default();
        if is_build_output(&name) {
            continue;
        }
        name.hash(hasher);
        hash_entry(&entry, hasher)?;
    }
    Ok(())
}
