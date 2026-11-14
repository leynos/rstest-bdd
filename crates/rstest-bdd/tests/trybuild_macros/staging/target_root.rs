//! Which Cargo target directory the trybuild run stages into and inspects.
//!
//! Kept apart from the staging code itself because the question is about
//! Cargo's layout, and that layout is explicitly not a stable interface. It has
//! already changed once under this module: Cargo 1.100.0 moved every compiled
//! unit, integration-test executables included, out of `<profile>/deps` into
//! `<profile>/build/<pkg>/<hash>/out`.

use std::sync::OnceLock;

use camino::{Utf8Path, Utf8PathBuf};
use cargo_metadata::MetadataCommand;
use rstest_bdd_harness::binary_test_support::target_directory_of;

/// Resolve the Cargo target directory shared by trybuild staging and inspection.
///
/// The running integration-test executable is compiled beneath Cargo's
/// effective target root, including a `--target-dir` selected by `cargo
/// llvm-cov`. Prefer that path over `cargo metadata`: metadata does not receive
/// a caller's command-line target-directory override.
pub(crate) fn trybuild_target_directory(workspace_root: &Utf8Path) -> Utf8PathBuf {
    static TARGET_DIRECTORY: OnceLock<Utf8PathBuf> = OnceLock::new();
    TARGET_DIRECTORY
        .get_or_init(|| {
            target_directory_from_running_test_executable()
                .unwrap_or_else(|| metadata_target_directory(workspace_root))
        })
        .clone()
}

/// Extract Cargo's target root from an integration-test executable path. Its
/// callers must use the returned root for both staged inputs and inspection
/// artefacts.
///
/// Found by its `CACHEDIR.TAG`, not by assuming the executable sits in `deps`:
/// Cargo 1.100.0 moved integration tests to `<profile>/build/<pkg>/<hash>/out`,
/// and the old form then returned `None`, so the metadata fallback silently
/// redirected staging to a tree excluding `llvm-cov`'s `--target-dir`.
fn target_directory_from_test_executable(executable: &Utf8Path) -> Option<Utf8PathBuf> {
    let root = target_directory_of(executable.as_std_path())?;
    Utf8PathBuf::from_path_buf(root).ok()
}

fn target_directory_from_running_test_executable() -> Option<Utf8PathBuf> {
    let executable = Utf8PathBuf::from_path_buf(std::env::current_exe().ok()?).ok()?;
    target_directory_from_test_executable(executable.as_path())
}

fn metadata_target_directory(workspace_root: &Utf8Path) -> Utf8PathBuf {
    match MetadataCommand::new().current_dir(workspace_root).exec() {
        Ok(metadata) => metadata.target_directory,
        Err(error) => {
            // This fallback is only for environments where neither the test
            // executable layout nor Cargo metadata is available, such as a
            // sandbox without a reachable manifest.
            let fallback = workspace_root.join("target");
            tracing::warn!(
                workspace_root = %workspace_root, error = %error,
                "trybuild target-directory fallback to `{fallback}` because cargo metadata failed: {error}"
            );
            fallback
        }
    }
}

#[cfg(test)]
#[path = "target_root/tests.rs"]
mod tests;
