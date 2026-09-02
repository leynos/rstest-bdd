//! File staging utilities for trybuild support.
//!
//! Handles discovery and copying of feature files to the trybuild test
//! environment, ensuring fixtures can locate their dependencies at compile time.
use std::{env, io, path::Path as StdPath, sync::OnceLock};

use camino::{Utf8Path, Utf8PathBuf};
use cap_std::{ambient_authority, fs::Dir};
use cargo_metadata::MetadataCommand;

const MACROS_FIXTURES_DIR: &str = "tests/fixtures_macros";
const FEATURES_DIR: &str = "tests/features";
const FEATURES_AUTO_DIR: &str = "tests/features/auto";

const TARGET_ROOT_SNAPSHOTS: &[&str] = &[
    "tests/fixtures_macros/scenario_missing_file.stderr",
    "tests/fixtures_macros/scenarios_autodiscovery_invalid_path.stderr",
    "tests/fixtures_macros/scenarios_missing_dir.stderr",
    "tests/fixtures_macros/scenario_unrelatable_path.stderr",
];
#[cfg(windows)]
const UNRELATABLE_FEATURE_PATH: &str = r"C:\Users\Public\rstest-bdd-unrelatable\x.feature";
/// A staged feature file on Windows' `C:` root.
///
/// Keeping the file alive through the compile-fail invocation ensures the macro
/// can load it before it checks the intentionally different filesystem root.
/// Dropping the guard removes this exact staged file and its empty directory.
#[cfg(windows)]
pub(crate) struct AlternateFeatureRoot;

#[cfg(windows)]
impl Drop for AlternateFeatureRoot {
    fn drop(&mut self) {
        let path = StdPath::new(UNRELATABLE_FEATURE_PATH);
        let _ = std::fs::remove_file(path);
        if let Some(parent) = path.parent() {
            let _ = std::fs::remove_dir(parent);
        }
    }
}
/// Returns the path to a macro fixture file, staging support files first.
///
/// Ensures that feature files are staged to the trybuild test environment
/// before returning the fixture path, so that macros can locate their
/// dependencies at compile time.
///
/// # Parameters
///
/// * `case` - The fixture file name (e.g., `"step_macros.rs"`).
///
/// # Returns
///
/// The full path to the fixture file as a [`Utf8PathBuf`].
pub(crate) fn macros_fixture(case: &str) -> Utf8PathBuf {
    ensure_trybuild_support_files();
    Utf8PathBuf::from(MACROS_FIXTURES_DIR).join(case)
}

/// Returns the path to a UI fixture file.
///
/// UI fixtures do not require feature file staging since they test
/// attribute macro diagnostics rather than scenario parsing.
///
/// # Parameters
///
/// * `case` - The fixture file name (e.g., `"datatable_wrong_type.rs"`).
///
/// # Returns
///
/// The full path to the fixture file as a [`Utf8PathBuf`].
pub(crate) fn ui_fixture(case: &str) -> Utf8PathBuf {
    Utf8PathBuf::from("tests/ui_macros").join(case)
}

/// Stage the unrelatable-path feature fixture on Windows' alternate root.
///
/// `#[scenario]` loads its feature before emitting its tracking item. Copying
/// the staged fixture to `C:` gives that load a real feature while the
/// trybuild crate remains on the hosted runner's `D:` workspace root.
#[cfg(windows)]
pub(crate) fn stage_unrelatable_feature_root() -> io::Result<Option<AlternateFeatureRoot>> {
    let crate_root = Utf8Path::new(env!("CARGO_MANIFEST_DIR"));
    let workspace_root = crate_root
        .parent()
        .and_then(Utf8Path::parent)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "workspace root must exist"))?;
    let target_root = trybuild_target_directory(workspace_root);
    if target_root.as_std_path().starts_with(r"C:\") {
        // Lading's publish preflight uses a temporary C: target. The fixed C:
        // fixture cannot exercise an unrelatable root there; the ordinary
        // Windows CI target is on D: and retains the compile-fail coverage.
        return Ok(None);
    }

    let source = crate_root.join("tests/fixtures_macros/unrelatable/x.feature");
    let destination = StdPath::new(UNRELATABLE_FEATURE_PATH);
    let Some(parent) = destination.parent() else {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "alternate feature path must have a parent directory",
        ));
    };
    std::fs::create_dir_all(parent)?;
    std::fs::copy(&source, destination)?;
    Ok(Some(AlternateFeatureRoot))
}
#[expect(clippy::expect_used, reason = "test setup failure should panic")]
fn ensure_trybuild_support_files() {
    use std::sync::OnceLock;
    static TRYBUILD_SUPPORT: OnceLock<()> = OnceLock::new();
    TRYBUILD_SUPPORT.get_or_init(|| {
        stage_trybuild_support_files().expect("failed to stage trybuild support files");
    });
}

fn stage_trybuild_support_files() -> io::Result<()> {
    let crate_root = Utf8Path::new(env!("CARGO_MANIFEST_DIR"));
    // This crate lives two levels down from the workspace root (workspace/crates/rstest-bdd),
    // so parent().and_then(parent) targets the workspace root. Update if the layout changes.
    let workspace_root = crate_root
        .parent()
        .and_then(Utf8Path::parent)
        .map(Utf8Path::to_owned)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "workspace root must exist"))?;
    let target_dir = trybuild_target_directory(&workspace_root);
    let target_dir_handle = Dir::open_ambient_dir(target_dir.as_std_path(), ambient_authority())?;

    let trybuild_crate_relative = Utf8Path::new("tests/trybuild/rstest-bdd");
    let workspace_features_relative = Utf8Path::new("tests/trybuild/features");

    remove_dir_if_exists(&target_dir_handle, workspace_features_relative)?;
    remove_dir_if_exists(&target_dir_handle, trybuild_crate_relative)?;

    target_dir_handle.create_dir_all(workspace_features_relative.as_std_path())?;
    target_dir_handle.create_dir_all(trybuild_crate_relative.as_std_path())?;

    let crate_dir = Dir::open_ambient_dir(crate_root.as_std_path(), ambient_authority())?;
    let features_dir = crate_dir.open_dir(FEATURES_DIR)?;
    let mut features = Vec::new();
    collect_feature_files(&features_dir, Utf8Path::new("."), &mut features)?;
    features.sort_by(|a, b| a.0.cmp(&b.0));

    let fixtures_dir = crate_dir.open_dir(MACROS_FIXTURES_DIR)?;
    let mut fixture_features = Vec::new();
    collect_feature_files(&fixtures_dir, Utf8Path::new("."), &mut fixture_features)?;
    fixture_features.sort_by(|a, b| a.0.cmp(&b.0));

    write_feature_files(
        &target_dir_handle,
        workspace_features_relative.as_std_path(),
        &features,
    )?;
    write_feature_files(
        &target_dir_handle,
        trybuild_crate_relative.as_std_path(),
        &fixture_features,
    )?;

    // Stage auto-discovery feature files for `scenarios!` compile-pass test.
    // Derive auto features as a subset of the main features list to avoid
    // re-walking the filesystem. Strip the "auto" prefix since the destination
    // directory already includes the "auto" segment. Use Utf8Path::strip_prefix
    // for cross-platform path separator handling.
    let auto_features: Vec<_> = features
        .into_iter()
        .filter_map(|(path, contents)| {
            Utf8Path::new(&path)
                .strip_prefix("auto")
                .ok()
                .map(|stripped| (stripped.to_string(), contents))
        })
        .collect();

    if !auto_features.is_empty() {
        let auto_dest = trybuild_crate_relative.join(FEATURES_AUTO_DIR);
        target_dir_handle.create_dir_all(auto_dest.as_std_path())?;
        write_feature_files(&target_dir_handle, auto_dest.as_std_path(), &auto_features)?;
    }

    Ok(())
}

/// Resolve the Cargo target directory shared by trybuild staging and inspection.
///
/// The running integration-test executable is compiled beneath Cargo's
/// effective target root, including a `--target-dir` selected by `cargo
/// llvm-cov`. Prefer that path over `cargo metadata`: metadata does not receive
/// a caller's command-line target-directory override.
pub(super) fn trybuild_target_directory(workspace_root: &Utf8Path) -> Utf8PathBuf {
    static TARGET_DIRECTORY: OnceLock<Utf8PathBuf> = OnceLock::new();
    TARGET_DIRECTORY
        .get_or_init(|| {
            target_directory_from_running_test_executable()
                .unwrap_or_else(|| metadata_target_directory(workspace_root))
        })
        .clone()
}

fn target_directory_from_running_test_executable() -> Option<Utf8PathBuf> {
    let executable = Utf8PathBuf::from_path_buf(env::current_exe().ok()?).ok()?;
    target_directory_from_test_executable(executable.as_path())
}

/// Extract Cargo's target root from an integration-test executable path.
///
/// This helper serves only the trybuild test-support module. Its callers must
/// use the returned root for both staged inputs and inspection artefacts.
fn target_directory_from_test_executable(executable: &Utf8Path) -> Option<Utf8PathBuf> {
    let dependencies = executable.parent()?;
    if dependencies.file_name() != Some("deps") {
        return None;
    }
    Some(dependencies.parent()?.parent()?.to_owned())
}

fn metadata_target_directory(workspace_root: &Utf8Path) -> Utf8PathBuf {
    match MetadataCommand::new().current_dir(workspace_root).exec() {
        Ok(metadata) => metadata.target_directory,
        Err(error) => {
            // This fallback is only for environments where neither the test
            // executable layout nor Cargo metadata is available, such as a
            // sandbox without a reachable manifest.
            log::warn!(
                "trybuild target-directory fallback to `{}` because cargo metadata failed: {error}",
                workspace_root.join("target")
            );
            workspace_root.join("target")
        }
    }
}

/// Restores snapshots whose diagnostics include Cargo's target directory.
///
/// Trybuild compares full diagnostics. When coverage selects a target
/// subdirectory, its otherwise identical feature-path diagnostics need that
/// root for the duration of the comparison. This guard restores checked-in
/// snapshots before the test returns.
pub(super) struct TargetRootSnapshotGuard {
    crate_root: Utf8PathBuf,
    originals: Vec<(Utf8PathBuf, String)>,
}

impl Drop for TargetRootSnapshotGuard {
    fn drop(&mut self) {
        let Ok(crate_dir) =
            Dir::open_ambient_dir(self.crate_root.as_std_path(), ambient_authority())
        else {
            return;
        };
        for (path, contents) in &self.originals {
            let _ = crate_dir.write(path.as_std_path(), contents.as_bytes());
        }
    }
}

/// Render a target root with the platform-independent snapshot separator.
fn snapshot_target_root(target_root: &Utf8Path, workspace_root: &Utf8Path) -> String {
    let native_target_root = target_root.as_str();
    let target_root = native_target_root.replace('\\', "/");
    let workspace_root = workspace_root.as_str().replace('\\', "/");
    let target_root_path = Utf8Path::new(&target_root);
    let workspace_root_path = Utf8Path::new(&workspace_root);
    let relative = target_root_path
        .strip_prefix(workspace_root_path)
        .ok()
        .map(Utf8Path::to_owned);

    relative.map_or_else(
        || native_target_root.to_owned(),
        |relative| format!("$WORKSPACE/{relative}"),
    )
}

fn apply_snapshot_target_root(original: &str, rendered_target_root: &str) -> String {
    const TRYBUILD_CRATE_ROOT: &str = "$WORKSPACE/target/tests/trybuild/rstest-bdd";

    if rendered_target_root.starts_with("$WORKSPACE/") {
        return original.replace("$WORKSPACE/target", rendered_target_root);
    }

    let separator = std::path::MAIN_SEPARATOR;
    let native_trybuild_root =
        [rendered_target_root, "tests", "trybuild", "rstest-bdd"].join(&separator.to_string());
    let source_with_boundary = format!("{TRYBUILD_CRATE_ROOT}/");
    let replacement_with_boundary = format!("{native_trybuild_root}{separator}");

    original
        .replace(&source_with_boundary, &replacement_with_boundary)
        .replace(TRYBUILD_CRATE_ROOT, &native_trybuild_root)
}

/// Stage temporary target-root-specific snapshots for a trybuild run.
pub(super) fn stage_target_root_snapshots() -> io::Result<TargetRootSnapshotGuard> {
    let crate_root = Utf8PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let workspace_root = crate_root
        .parent()
        .and_then(Utf8Path::parent)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "workspace root must exist"))?;
    let target_root = trybuild_target_directory(workspace_root);
    let snapshot_target_root = snapshot_target_root(&target_root, workspace_root);
    let crate_dir = Dir::open_ambient_dir(crate_root.as_std_path(), ambient_authority())?;
    let mut originals = Vec::new();

    for path in TARGET_ROOT_SNAPSHOTS {
        let path = Utf8PathBuf::from(path);
        let original = crate_dir.read_to_string(path.as_std_path())?;
        originals.push((path, original));
    }

    let guard = TargetRootSnapshotGuard {
        crate_root,
        originals,
    };
    for (path, original) in &guard.originals {
        let adjusted = apply_snapshot_target_root(original, snapshot_target_root.as_str());
        crate_dir.write(path.as_std_path(), adjusted.as_bytes())?;
    }
    Ok(guard)
}
fn write_feature_files(
    root: &Dir,
    destination_root: &StdPath,
    features: &[(String, String)],
) -> io::Result<()> {
    let destination_root =
        Utf8PathBuf::from_path_buf(destination_root.to_path_buf()).map_err(|_| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "destination_root must be valid UTF-8",
            )
        })?;

    for (relative, contents) in features {
        let path = destination_root.join(relative);
        if let Some(parent) = path.parent() {
            root.create_dir_all(parent.as_std_path())?;
        }
        root.write(path.as_std_path(), contents.as_bytes())?;
    }

    Ok(())
}

fn remove_dir_if_exists(root: &Dir, path: &Utf8Path) -> io::Result<()> {
    match root.remove_dir_all(path.as_std_path()) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    }
}

fn collect_feature_files(
    dir: &Dir,
    current: &Utf8Path,
    features: &mut Vec<(String, String)>,
) -> io::Result<()> {
    let is_root = current == Utf8Path::new(".");
    for entry in dir.read_dir(current.as_std_path())? {
        let entry = entry?;
        let file_name = entry
            .file_name()
            .to_str()
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("file name is not valid UTF-8: {:?}", entry.file_name()),
                )
            })?
            .to_owned();
        let relative = if is_root {
            Utf8PathBuf::from(file_name.as_str())
        } else {
            current.join(file_name.as_str())
        };

        if entry.file_type()?.is_dir() {
            collect_feature_files(dir, relative.as_path(), features)?;
            continue;
        }

        if !file_name.ends_with(".feature") {
            continue;
        }

        let contents = dir.read_to_string(relative.as_std_path())?;
        features.push((relative.to_string(), contents));
    }

    Ok(())
}

#[cfg(test)]
#[path = "staging/tests.rs"]
mod tests;
