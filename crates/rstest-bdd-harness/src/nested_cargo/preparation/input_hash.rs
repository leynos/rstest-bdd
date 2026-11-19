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

mod config;

// Re-exported at the crate's public surface through `preparation`, so this
// cannot be narrower than `pub`: `input_hash` exists to hash what a caller
// names, and discovering Cargo's own configuration is part of that list.
pub use config::cargo_config_inputs;
// The two seams the configuration cases vary instead of the process's
// environment. Both are reached only from `#[cfg(test)]` modules, so a release
// build carries neither name.
#[cfg(test)]
pub(crate) use config::{cargo_config_inputs_with, resolve_cargo_home};

use super::stamp::{PREPARED_FILE_SUFFIX, STAMP_VERSION};

/// Directory names that hold build output rather than fixture inputs.
///
/// `target` holds the nested build's artefacts, `wip` holds `trybuild`'s
/// scratch files and `.git` holds version control state. None of them changes
/// what the fixture compiles.
const BUILD_OUTPUT_ENTRIES: [&str; 3] = ["target", "wip", ".git"];

/// Suffix of the profile files an instrumented test binary writes.
///
/// `-Cinstrument-coverage` makes every test binary drop a
/// `<name>_<hash>_<pid>.profraw` beside the working directory it ran in, and
/// the working directory here is the crate whose test just ran — inside the
/// `crates` root this walk hashes. The name carries the process id, so each
/// instrumented run writes a *new* file rather than overwriting one: the tree
/// changes on every covered run without a source byte changing.
///
/// A coverage profile records what the test executed. It is runtime output,
/// exactly like `target`, and nothing the fixture compiles reads it. Counting
/// it made a stamp written by the preparation stale by the time its consumers
/// ran under `cargo llvm-cov` — the preparation wrote the stamp, then the
/// instrumented binaries it had just been asked about added profiles, and the
/// gate refused the stamp with an empty evicted list, which is the signature
/// of a fingerprint mismatch rather than missing artefacts. Hosted coverage
/// runs hit this on all three lanes; a plain run never writes a profile at
/// all, so it could not reproduce.
const COVERAGE_PROFILE_SUFFIX: &str = ".profraw";

/// Whether an entry name is build output rather than an input.
///
/// The stamp files the preparation module writes are excluded too: they name
/// the fingerprint computed from these inputs, so counting them would make
/// every preparation invalidate the fingerprint that describes it.
fn is_build_output(name: &str) -> bool {
    BUILD_OUTPUT_ENTRIES.contains(&name)
        || name.ends_with(PREPARED_FILE_SUFFIX)
        || name.ends_with(COVERAGE_PROFILE_SUFFIX)
}

/// Marker hashed where a link re-enters a directory already on the walk.
///
/// Without it a cyclic descent and an empty directory would contribute the
/// same bytes, so two genuinely different trees would share a hash. The marker
/// is a fixed string rather than the resolved path of the directory it cuts,
/// because the hash has to be reproducible from the checkout alone.
const CYCLE_MARKER: &str = "symlink-cycle";

/// Marker hashed once a directory's entries have all been walked.
///
/// Without it the walk records names and contents but never records where one
/// directory's entries stopped and the next began, so two different trees can
/// feed the hasher the same sequence. Moving `tests/check.rs` to
/// `src/tests/check.rs` beside an unchanged `src/lib.rs` is exactly that: both
/// trees emit `src`, `lib.rs`, `tests` and `check.rs`, in that order, and
/// nothing in either stream says whether `tests` sits beside `src` or inside
/// it. A fixed string rather than a path, for the reason [`CYCLE_MARKER`]
/// gives: the hash has to be reproducible from the checkout, and a resolved
/// path would also make the stamp depend on the platform's separator.
const DIRECTORY_END: &str = "directory-end";

/// Hash the inputs a preparation family's artefacts derive from.
///
/// `roots` may name files or directories; directories are walked in sorted
/// order and every regular file contributes its relative path, length and
/// bytes. A symbolic link contributes its own name and, through the link, the
/// contents of its target, because that is what Cargo reads. Directory entries
/// named `target`, `.git` or `wip` are skipped, as are the stamp files the
/// preparation module writes and the `.profraw` profiles an instrumented test
/// binary leaves behind, so a nested build never invalidates the fingerprint
/// that describes it.
///
/// Each entry contributes what kind of entry it is, and each directory
/// contributes `DIRECTORY_END` after its last entry, so the walk records the
/// shape of the tree and not only the sequence of names in it. Without those
/// markers, moving `tests/check.rs` to `src/tests/check.rs` beside an unchanged
/// `src/lib.rs` produces the same stream of names and contents — `src`,
/// `lib.rs`, `tests`, `check.rs` — and the stamp would survive a move that
/// changes which files the fixture compiles.
///
/// `wip` is excluded because `trybuild` writes its scratch directory there,
/// beside the crate under test rather than inside the target directory. The
/// preparation computes this hash before it runs the build that creates that
/// directory, so counting it would make every stamp stale the moment it was
/// written.
///
/// Following a link means the walk can be led back to a directory it is
/// already inside; the walk cuts the descent there, so a cyclic tree is hashed
/// rather than walked for ever.
///
/// # Parameters
/// - `roots`: The files and directories the preparation's artefacts derive from, in the order the
///   caller wants them hashed.
///
/// # Returns
/// The sixteen-digit hexadecimal digest of every entry beneath `roots`. It is
/// stable across processes and platforms: nothing platform-specific — a
/// resolved path, a separator or a `read_dir` order — reaches the hasher.
///
/// # Errors
///
/// Returns the read error when a named input or one of its files cannot be
/// read.
pub fn input_hash(roots: &[PathBuf]) -> io::Result<String> {
    let mut hasher = DefaultHasher::new();
    STAMP_VERSION.hash(&mut hasher);
    let mut ancestors = Vec::new();
    for root in roots {
        hash_entry(root, &mut hasher, &mut ancestors)?;
    }
    Ok(format!("{:016x}", hasher.finish()))
}

/// Markers naming what an entry is, hashed before its body.
///
/// A directory and a file of the same name are different inputs — one holds
/// further entries, the other holds bytes — but neither their names nor their
/// bodies have to say so. An empty file contributes only a zero length and an
/// empty directory contributes only `DIRECTORY_END`, and treating those two
/// as the same would let a stamp survive an entry changing kind. Naming the
/// kind makes that distinction explicit instead of incidental, and does it
/// without depending on the platform's separator or on `read_dir` order.
const KIND_FILE: &str = "file";
/// See [`KIND_FILE`].
const KIND_DIRECTORY: &str = "directory";
/// See [`KIND_FILE`].
const KIND_SYMLINK: &str = "symlink";
/// See [`KIND_FILE`].
const KIND_OTHER: &str = "other";

/// Feed one file or directory tree into `hasher`.
fn hash_entry(
    path: &Path,
    hasher: &mut impl Hasher,
    ancestors: &mut Vec<PathBuf>,
) -> io::Result<()> {
    let entry = fs::symlink_metadata(path)?;
    // The kind is recorded before anything the entry contributes, so the same
    // name cannot mean two different things in two trees.
    if entry.file_type().is_symlink() {
        KIND_SYMLINK.hash(hasher);
        return hash_symlink(path, hasher, ancestors);
    }
    if entry.is_file() {
        KIND_FILE.hash(hasher);
        path.file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .hash(hasher);
        // A *named* root also contributes its full path, because two files
        // with the same name and the same bytes can still mean different
        // things. Cargo configuration is the case that matters: a
        // `build.rustc = "tools/rustc"` resolves relative to the configuration
        // file's own directory, so relocating an identical `/work/.cargo/
        // config.toml` to `/.cargo/config.toml` selects a different compiler
        // while leaving the name and the bytes unchanged. A stamp that hashed
        // only those two would survive the move and admit a timed build that
        // recompiles the graph.
        //
        // Roots handed in from a fixture tree are already covered by their
        // relative name, and their absolute path is deliberately *not* hashed:
        // the hash has to be reproducible from the checkout, so a stamp must
        // not depend on where the checkout lives. Only a root the caller named
        // explicitly is recorded, and callers name configuration files by
        // absolute path precisely because where they live is what gives them
        // meaning.
        if ancestors.is_empty() {
            PROVENANCE_KIND.hash(hasher);
            path.to_string_lossy().hash(hasher);
        }
        let contents = fs::read(path)?;
        contents.hash(hasher);
        return Ok(());
    }
    if !entry.is_dir() {
        // A socket or a pipe: nothing a fixture build reads, but its arrival
        // beside an input is still a change to the tree.
        KIND_OTHER.hash(hasher);
        return Ok(());
    }
    KIND_DIRECTORY.hash(hasher);
    hash_directory(path, hasher, ancestors)
}

/// Hash the provenance of a file the caller named as a root itself.
///
/// Every root passed to [`input_hash`] is hashed by its own path, and the walk
/// beneath a directory root is not: a directory's entries are hashed by the
/// name the tree gives them, so the hash stays reproducible from the checkout.
/// A root named as a file is different, because nothing above it is hashed at
/// all — and for a Cargo configuration file, where the file lives is part of
/// what it means. `build.rustc` and other path-valued keys resolve against the
/// configuration's own directory, so the same bytes in a different directory
/// select a different compiler.
///
/// This is the provenance half of the reading; see [`hash_entry`] for where it
/// is applied.
const PROVENANCE_KIND: &str = "named-root";

/// Hash a symbolic link as the target Cargo would read through it.
///
/// [`fs::symlink_metadata`] classifies a link as neither file nor directory, so
/// hashing it as one would contribute nothing: a `.cargo/config.toml` committed
/// as a symlink would be left out of the fingerprint entirely, and editing the
/// file it points at would leave the stamp matching — the configuration-only
/// invalidation [`cargo_config_inputs`] exists to force. The link's own name is
/// recorded as well, because removing or renaming it changes what Cargo
/// discovers even when the target is untouched.
///
/// A link is resolved with [`fs::metadata`], which follows, and its target is
/// then walked as the directory or file Cargo would read. Walking it calls
/// back into [`hash_directory`], which cuts a descent that re-enters a
/// directory already on the stack — a link can point at an ancestor, and
/// following that without a bound would never terminate. A dangling link is
/// therefore a read error rather than a silent absence, matching how the rest
/// of this module treats an entry that exists but cannot be read.
fn hash_symlink(
    path: &Path,
    hasher: &mut impl Hasher,
    ancestors: &mut Vec<PathBuf>,
) -> io::Result<()> {
    let target = fs::metadata(path)?;
    path.file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .hash(hasher);
    if target.is_dir() {
        return hash_directory(path, hasher, ancestors);
    }
    if target.is_file() {
        let contents = fs::read(path)?;
        contents.hash(hasher);
    }
    Ok(())
}

/// Walk `directory` in sorted order, hashing each entry.
///
/// The directory's resolved identity is pushed for the duration of the walk so
/// a link back into it is recognized. Resolution happens on the identity only
/// — the paths handed to [`hash_entry`] stay as the tree names them, so the
/// hash keeps depending on the checkout rather than on where it is mounted.
fn hash_directory(
    directory: &Path,
    hasher: &mut impl Hasher,
    ancestors: &mut Vec<PathBuf>,
) -> io::Result<()> {
    let identity = fs::canonicalize(directory)?;
    if ancestors.contains(&identity) {
        CYCLE_MARKER.hash(hasher);
        return Ok(());
    }
    ancestors.push(identity);
    let entries: Vec<PathBuf> = fs::read_dir(directory)?
        .map(|entry| entry.map(|entry| entry.path()))
        .collect::<io::Result<_>>()?;
    let outcome = hash_sorted(entries, hasher, ancestors);
    ancestors.pop();
    outcome
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
///
/// Visible to the child test module, which is the only caller that supplies a
/// sequence rather than letting the walk list a directory.
pub(super) fn hash_sorted(
    mut entries: Vec<PathBuf>,
    hasher: &mut impl Hasher,
    ancestors: &mut Vec<PathBuf>,
) -> io::Result<()> {
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
        hash_entry(&entry, hasher, ancestors)?;
    }
    // Emitted after every entry, not before the next one: an empty directory
    // and a directory whose last entry was a file both end here, which is what
    // makes the marker name a *close* rather than a sibling boundary. The
    // producing caller hashes the directory's own name first, so the pair
    // `name` + `directory-end` brackets exactly one listing.
    DIRECTORY_END.hash(hasher);
    Ok(())
}

// The unit tests live beside this file so the walk they exercise keeps its own
// file under the repository's line budget.
#[cfg(test)]
#[path = "input_hash/tests.rs"]
mod tests;
