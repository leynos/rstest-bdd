//! Shared helpers for harness crates that run `macrotest` and `trybuild`
//! against fixture crates.
//!
//! These utilities back the `macro_compile` integration suites in the Tokio
//! and GPUI harness crates so both can resolve `cargo expand`, decide whether
//! macrotest refresh is enabled, perform substring assertions over
//! `.expanded.rs` snapshot files, and prepare the trybuild build tree their
//! compile-time cases run in without re-implementing identical code in each
//! crate. The module is exposed as a hidden API for those harness tests only
//! and is not part of the supported public surface.
//!
//! Snapshot paths are passed as absolute paths because each caller's
//! `CARGO_MANIFEST_DIR` resolves at compile time inside the caller crate;
//! sharing the helpers via this module avoids that per-crate coupling.

use std::{
    error::Error,
    fs,
    io,
    path::{Path, PathBuf},
    process::Command,
};

use crate::nested_cargo::{self, Preparation, PreparationSummary};

/// Returns `true` when the macrotest refresh workflow is enabled.
///
/// The workflow requires the `RSTEST_BDD_RUN_MACROTEST` environment variable
/// to be set and the `cargo expand` subcommand to be available. Both
/// conditions must hold for `macrotest::expand_without_refresh` to produce a
/// useful comparison against the checked-in `.expanded.rs` files.
#[must_use]
pub fn snapshot_refresh_is_enabled() -> bool {
    std::env::var_os("RSTEST_BDD_RUN_MACROTEST").is_some() && cargo_expand_is_available()
}

fn cargo_expand_is_available() -> bool {
    Command::new("cargo")
        .args(["expand", "--version"])
        .output()
        .is_ok_and(|output| output.status.success())
}

/// Reads a snapshot file at `path`, panicking with file context on failure.
fn read_snapshot(path: &Path) -> String {
    match fs::read_to_string(path) {
        Ok(contents) => contents,
        Err(err) => panic!("failed to read snapshot {}: {err}", path.display()),
    }
}

/// Asserts that every needle appears at least once in the snapshot at `path`.
///
/// # Panics
///
/// Panics if `path` cannot be read (I/O error), or if any needle is absent
/// from the snapshot contents.
pub fn assert_snapshot_contains(path: &Path, needles: &[&str]) {
    let contents = read_snapshot(path);
    let missing = needles
        .iter()
        .copied()
        .filter(|needle| !contents.contains(needle))
        .collect::<Vec<_>>();
    assert!(
        missing.is_empty(),
        "expected {} to contain {missing:?}",
        path.display(),
    );
}

/// Asserts that `needle` does not appear anywhere in the snapshot at `path`.
///
/// # Panics
///
/// Panics if `path` cannot be read (I/O error), or if `needle` is found in
/// the snapshot contents.
pub fn assert_snapshot_omits(path: &Path, needle: &str) {
    let contents = read_snapshot(path);
    assert!(
        !contents.contains(needle),
        "expected {} to omit {needle:?}",
        path.display(),
    );
}

/// Computes the Cargo target root trybuild builds its generated crates into.
///
/// `trybuild` asks `cargo metadata` for the target directory and then sets
/// `CARGO_TARGET_DIR` to `<target>/tests/trybuild` for every generated crate.
/// That directory, not the workspace `target`, is the artefact directory a
/// preparation has to warm and stamp.
///
/// Resolution goes through `cargo metadata` so the preparation and the timed
/// cases describe the same tree: metadata reads `CARGO_TARGET_DIR` from the
/// environment, so a coverage driver that exports it is followed, while an
/// outer `--target-dir` flag is invisible to both and therefore still leaves
/// them in step with each other.
///
/// # Errors
///
/// Returns the `cargo metadata` error when the manifest cannot be resolved.
pub fn trybuild_target_root(manifest_path: &Path) -> Result<PathBuf, Box<dyn Error>> {
    let metadata = cargo_metadata::MetadataCommand::new()
        .manifest_path(manifest_path)
        .no_deps()
        .exec()?;
    Ok(metadata
        .target_directory
        .into_std_path_buf()
        .join("tests/trybuild"))
}

/// Computes the trybuild scratch directory for the crate whose manifest lives
/// at `manifest_path`.
///
/// The returned path joins the shared trybuild root with `target_subdir`,
/// matching the layout trybuild uses for per-crate scratch space.
///
/// # Errors
///
/// As [`trybuild_target_root`].
pub fn trybuild_crate_root(
    manifest_path: &Path,
    target_subdir: &str,
) -> Result<PathBuf, Box<dyn Error>> {
    Ok(trybuild_target_root(manifest_path)?.join(target_subdir))
}

/// One harness crate's trybuild build tree, prepared before its timed cases.
///
/// The timed `macro_compile` cases in the Tokio and GPUI harness crates, and
/// the server crate's compile test, each drive `trybuild` against their own
/// generated crate. On a cold target directory that run compiles the crate's
/// whole dependency closure inside the test's own deadline, which is the
/// failure mode issue 910 records. A named preparation test compiles one
/// compile-pass fixture into the same tree first, and the timed case gates on
/// the stamp it writes.
///
/// All three crates build into the *same* `<target>/tests/trybuild` directory,
/// because `trybuild` derives it from the workspace target root rather than
/// from the generated crate. Each therefore stamps that shared directory under
/// its own family name, so warming one tree also warms the next crate's
/// dependencies without letting one crate's stamp vouch for another's.
pub struct TrybuildTree {
    /// Directory holding the crate whose trybuild tree is prepared.
    crate_root: PathBuf,
    /// Subdirectory of the shared trybuild root holding the generated crate.
    target_subdir: String,
    /// Stable kebab-case family name used in the stamp file name and logs.
    family: String,
    /// Label for the feature selection the generated crate is built with.
    variant: String,
}

impl TrybuildTree {
    /// Describe the trybuild tree belonging to `crate_root`.
    ///
    /// `family` must be distinct per crate: the tree the stamp protects is
    /// shared, but what each crate's generated manifest compiles is not.
    /// `variant` records the feature selection this crate's test binary was
    /// built with, because `trybuild` propagates the binary's enabled features
    /// to the generated crate and a different selection is a different graph.
    #[must_use]
    pub fn new(
        crate_root: &Path,
        target_subdir: impl Into<String>,
        family: impl Into<String>,
        variant: impl Into<String>,
    ) -> Self {
        Self {
            crate_root: crate_root.to_path_buf(),
            target_subdir: target_subdir.into(),
            family: family.into(),
            variant: variant.into(),
        }
    }

    /// Prepare the tree unconditionally, for the named preparation test.
    ///
    /// Still idempotent: a warm tree costs one stamp read. This is the entry
    /// point `make prepare-nested-fixtures` reaches, which runs under nextest
    /// where [`Self::ensure_prepared`] would otherwise fail fast.
    ///
    /// # Errors
    ///
    /// Returns the closure's error when the probe compile fails, the I/O error
    /// when the stamp cannot be read or written, and a metadata error when the
    /// shared trybuild root cannot be resolved.
    pub fn prepare_now<F>(&self, prepare: F) -> io::Result<()>
    where
        F: FnOnce() -> io::Result<PreparationSummary>,
    {
        nested_cargo::prepare_now(&self.preparation()?, prepare)
    }

    /// Ensure the tree is prepared, failing fast under nextest when it is not.
    ///
    /// # Errors
    ///
    /// Returns a [`io::ErrorKind::NotFound`] error naming the preparation
    /// command when the stamp is missing or stale under nextest, the closure's
    /// error when lazy preparation under plain `cargo test` fails, and the
    /// stamp I/O error when it cannot be written.
    pub fn ensure_prepared<F>(&self, prepare: F) -> io::Result<()>
    where
        F: FnOnce() -> io::Result<PreparationSummary>,
    {
        nested_cargo::ensure_prepared(&self.preparation()?, prepare)
    }

    /// The generated crate's directory under the shared trybuild root.
    ///
    /// Callers resolve support files (`.feature` inputs) against this path
    /// before preparing, because `trybuild` reads them from there.
    ///
    /// # Errors
    ///
    /// Returns the `cargo metadata` error when the manifest cannot be resolved.
    pub fn generated_crate_root(&self) -> Result<PathBuf, Box<dyn Error>> {
        trybuild_crate_root(&self.manifest_path(), &self.target_subdir)
    }

    /// The manifest of the crate whose trybuild tree this is.
    fn manifest_path(&self) -> PathBuf { self.crate_root.join("Cargo.toml") }

    /// Describe the family this tree belongs to.
    fn preparation(&self) -> io::Result<Preparation> {
        let manifest_path = self.manifest_path();
        let target_root = trybuild_target_root(&manifest_path).map_err(|error| {
            io::Error::other(format!(
                "resolving the trybuild target root for {}: {error}",
                manifest_path.display()
            ))
        })?;
        Ok(Preparation::for_fixture(
            self.family.clone(),
            target_root,
            self.variant.clone(),
            nested_cargo::input_hash(&self.inputs())?,
        ))
    }

    /// The inputs whose change invalidates the prepared tree.
    ///
    /// The generated crate is built inside the workspace, so its dependency
    /// graph comes from the workspace manifest and lockfile, the workspace
    /// crates it depends on, and any vendored path dependencies. Missing
    /// optional roots are skipped rather than treated as an error, and
    /// `input_hash` skips every `target` directory, so build output never
    /// invalidates its own stamp.
    fn inputs(&self) -> Vec<PathBuf> {
        let Some(workspace_root) = self
            .crate_root
            .parent()
            .and_then(Path::parent)
            .map(Path::to_path_buf)
        else {
            return vec![self.crate_root.clone()];
        };
        ["Cargo.toml", "Cargo.lock", "crates", "vendor"]
            .into_iter()
            .map(|entry| workspace_root.join(entry))
            .filter(|path| path.exists())
            .collect()
    }
}
