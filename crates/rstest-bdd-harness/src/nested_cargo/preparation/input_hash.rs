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

/// Paths of the Cargo configuration files committed at `workspace_root`.
///
/// Cargo discovers configuration by walking from a command's working directory
/// up to the filesystem root, so a nested fixture build beneath the workspace
/// applies the workspace `.cargo/config.toml`. That file sets development
/// profile options and the Linux linker flags, and changing either changes
/// dependency artefacts without changing a single fixture source. A stamp that
/// ignored it would survive a configuration-only edit, and the timed assertion
/// would then compile the graph the preparation was meant to have built.
///
/// Only the two files at the workspace root are reported. Configuration deeper
/// in the tree is already covered, because every caller also hashes the
/// `crates/` tree and its own fixture directory, and [`input_hash`] walks both.
/// Configuration above the workspace — `CARGO_HOME` and a user's home
/// directory — is host state rather than part of the checkout, so it is
/// deliberately excluded: a stamp has to be reproducible from the repository
/// alone.
///
/// Cargo accepts `config.toml` or the legacy `config` name, and a checkout may
/// commit neither, so missing files are skipped.
#[must_use]
pub fn cargo_config_inputs(workspace_root: &Path) -> Vec<PathBuf> {
    [".cargo/config.toml", ".cargo/config"]
        .into_iter()
        .map(|entry| workspace_root.join(entry))
        .filter(|path| path.is_file())
        .collect()
}

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
    let entries: Vec<PathBuf> = fs::read_dir(path)?
        .map(|entry| entry.map(|entry| entry.path()))
        .collect::<io::Result<_>>()?;
    hash_sorted(entries, hasher)
}

/// Hash `entries` in sorted order, skipping build output.
///
/// `read_dir` order is the filesystem's, not a property of the tree: on
/// ext4 it is a hash of the names, so the same tree lists in the same order
/// on every machine but a *different* order than a name-sorted walk would
/// produce. Feeding that order to the hasher would make the stamp depend on
/// where the fixture was built. Separating the sort from the traversal lets
/// a test supply a known-unsorted sequence, which no sequence of file
/// creations on a real filesystem can force.
fn hash_sorted(mut entries: Vec<PathBuf>, hasher: &mut impl Hasher) -> io::Result<()> {
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

#[cfg(test)]
mod tests {
    //! Unit tests for the sorted walk, which the property tests cannot see.

    use std::{collections::hash_map::DefaultHasher, fs, hash::Hasher};

    use super::hash_sorted;

    /// The same entries hash the same whatever order the directory listed
    /// them in.
    ///
    /// This cannot be expressed by creating a tree two ways. On ext4 —
    /// measured on this host — `read_dir` returns entries in a hash of their
    /// names, independent of creation order and of how many times the
    /// directory grew, so two trees with the same names and contents list
    /// identically however they were written. A test written that way passes
    /// even when the sort is removed, which was verified by replacing the
    /// sort with a reversal and watching it still pass.
    ///
    /// Handing the walk a known-unsorted sequence is the only way to see the
    /// difference: this fails immediately if the sort goes away.
    #[test]
    fn entries_hash_in_sorted_order_not_listing_order() {
        let directory = tempfile::tempdir().expect("temp dir");
        let mut entries = Vec::new();
        for name in ["zeta", "alpha", "mid", "beta"] {
            let path = directory.path().join(name);
            fs::write(&path, name.as_bytes()).expect("input file");
            entries.push(path);
        }

        let mut listed = DefaultHasher::new();
        let listed_order = entries.clone();
        hash_sorted(listed_order, &mut listed).expect("listed order");

        entries.sort();
        let mut sorted = DefaultHasher::new();
        hash_sorted(entries, &mut sorted).expect("sorted order");

        assert_eq!(
            listed.finish(),
            sorted.finish(),
            "the walk must sort: a directory listing is the filesystem's order"
        );
    }

    /// Build output is skipped wherever it appears in the sequence, including
    /// after a sort that puts it ahead of the inputs.
    #[test]
    fn build_output_is_skipped_in_every_position() {
        let directory = tempfile::tempdir().expect("temp dir");
        let input = directory.path().join("feature");
        fs::write(&input, b"Feature: one").expect("input file");
        let mut entries = vec![input.clone()];

        let mut before = DefaultHasher::new();
        hash_sorted(entries.clone(), &mut before).expect("without build output");

        let output = directory.path().join("target");
        fs::create_dir_all(&output).expect("target dir");
        fs::write(output.join("output"), b"binary").expect("artefact");
        entries.push(output);
        entries.push(directory.path().join("wip"));

        let mut after = DefaultHasher::new();
        hash_sorted(entries, &mut after).expect("with build output");

        assert_eq!(
            before.finish(),
            after.finish(),
            "target and wip must not reach the hash"
        );
    }
}
