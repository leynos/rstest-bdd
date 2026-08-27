//! Crate-root discovery using cargo metadata.
//!
//! This module provides functionality for discovering workspace information
//! from a given path using `cargo metadata`. It identifies the workspace root,
//! package names, and feature file locations.

use std::{
    io,
    path::{Path, PathBuf},
    time::Instant,
};

use cargo_metadata::MetadataCommand;
use metrics::{counter, describe_counter, describe_histogram, histogram};
use tracing::{info_span, warn};

use crate::error::ServerError;

/// Metric name for feature-file discovery outcomes.
const FEATURE_FILE_DISCOVERY_COUNTER: &str = "rstest_bdd_server_feature_file_discovery_total";
/// Metric name for feature-file discovery duration.
const FEATURE_FILE_DISCOVERY_DURATION: &str =
    "rstest_bdd_server_feature_file_discovery_duration_seconds";
/// Fixed operation name shared by feature-file discovery telemetry.
const FEATURE_FILE_DISCOVERY_OPERATION: &str = "feature_file_discovery";

/// Information about a discovered workspace.
///
/// Contains the workspace root path and the names of packages within the
/// workspace.
#[derive(Debug, Clone)]
pub struct WorkspaceInfo {
    /// Path to the workspace root (directory containing the root Cargo.toml).
    pub root: PathBuf,
    /// Package names within the workspace.
    pub packages: Vec<String>,
}

/// Discover workspace information from a given path.
///
/// Uses `cargo metadata` to find the workspace root and enumerate packages.
/// The path can be any file or directory within the workspace.
///
/// # Arguments
///
/// * `path` - A path within the workspace (file or directory)
///
/// # Errors
///
/// Returns `ServerError::CargoMetadata` if the metadata command fails, or
/// `ServerError::WorkspaceDiscovery` if the path is not within a Cargo
/// workspace.
///
/// # Examples
///
/// ```ignore
/// use std::path::Path;
/// use rstest_bdd_server::discovery::discover_workspace;
///
/// let info = discover_workspace(Path::new("/path/to/project"))?;
/// println!("Workspace root: {}", info.root.display());
/// ```
pub fn discover_workspace(path: &Path) -> Result<WorkspaceInfo, ServerError> {
    let manifest_path = find_manifest_path(path)?;

    let metadata = MetadataCommand::new()
        .manifest_path(&manifest_path)
        .no_deps()
        .exec()?;

    let packages = metadata.packages.iter().map(|p| p.name.clone()).collect();

    Ok(WorkspaceInfo {
        root: metadata.workspace_root.into_std_path_buf(),
        packages,
    })
}

/// Find the nearest Cargo.toml manifest file from a given path.
///
/// Walks up the directory tree from the given path until a Cargo.toml is found.
///
/// # Errors
///
/// Returns `ServerError::WorkspaceDiscovery` if no Cargo.toml is found.
fn find_manifest_path(path: &Path) -> Result<PathBuf, ServerError> {
    let start = if path.is_file() {
        path.parent().unwrap_or(path)
    } else {
        path
    };

    let mut current = start;
    loop {
        let manifest = current.join("Cargo.toml");
        if manifest.exists() {
            return Ok(manifest);
        }
        match current.parent() {
            Some(parent) => current = parent,
            None => {
                return Err(ServerError::WorkspaceDiscovery(format!(
                    "no Cargo.toml found in {} or any parent directory",
                    start.display()
                )));
            }
        }
    }
}

/// Find all `.feature` files within a workspace.
///
/// Searches common locations for Gherkin feature files:
/// - `tests/features/`
/// - `features/`
/// - Any subdirectory matching `**/features/*.feature`
///
/// # Arguments
///
/// * `workspace_root` - The root directory of the workspace
///
/// # Errors
///
/// Returns `ServerError::Io` when the workspace or a feature directory cannot
/// be read, including failures encountered while iterating directory entries.
///
/// # Examples
///
/// ```ignore
/// use std::path::Path;
/// use rstest_bdd_server::discovery::find_feature_files;
///
/// let features = find_feature_files(Path::new("/path/to/project"))?;
/// for path in features {
///     println!("Found feature: {}", path.display());
/// }
/// ```
pub fn find_feature_files(workspace_root: &Path) -> Result<Vec<PathBuf>, ServerError> {
    describe_counter!(
        FEATURE_FILE_DISCOVERY_COUNTER,
        "Feature-file discovery outcomes, labelled by bounded outcome"
    );
    describe_histogram!(
        FEATURE_FILE_DISCOVERY_DURATION,
        "Elapsed feature-file discovery time in seconds"
    );

    let span = info_span!(
        FEATURE_FILE_DISCOVERY_OPERATION,
        workspace_root = %workspace_root.display(),
        elapsed_seconds = tracing::field::Empty,
    );
    let _span_guard = span.enter();
    let started = Instant::now();
    let result = find_feature_files_with(workspace_root, &StandardDirectoryReader);
    let elapsed_seconds = started.elapsed().as_secs_f64();

    span.record("elapsed_seconds", elapsed_seconds);
    histogram!(FEATURE_FILE_DISCOVERY_DURATION).record(elapsed_seconds);

    let outcome = if result.is_ok() {
        "success"
    } else {
        "io-failure"
    };
    counter!(FEATURE_FILE_DISCOVERY_COUNTER, "outcome" => outcome).increment(1);

    if let Err(ServerError::Io(error)) = &result {
        warn!(
            operation = FEATURE_FILE_DISCOVERY_OPERATION,
            workspace_root = %workspace_root.display(),
            error = %error,
            elapsed_seconds,
            "feature file discovery failed"
        );
    }

    result
}

/// Provides the directory operations required by workspace discovery.
trait DirectoryReader {
    /// The iterator returned by [`DirectoryReader::read_dir`].
    type Entries: Iterator<Item = io::Result<PathBuf>>;

    /// Reads a directory, preserving errors encountered while iterating it.
    fn read_dir(&self, path: &Path) -> io::Result<Self::Entries>;

    /// Reports whether a path names a directory, or returns its metadata error.
    fn is_directory(&self, path: &Path) -> io::Result<bool>;

    /// Reports whether a path names a file, or returns its metadata error.
    fn is_file(&self, path: &Path) -> io::Result<bool>;
}

/// Uses the standard filesystem APIs to read workspace directories.
struct StandardDirectoryReader;

/// Iterator that maps standard directory entries to their paths.
type StandardDirectoryEntries =
    std::iter::Map<std::fs::ReadDir, fn(io::Result<std::fs::DirEntry>) -> io::Result<PathBuf>>;

impl DirectoryReader for StandardDirectoryReader {
    type Entries = StandardDirectoryEntries;

    fn read_dir(&self, path: &Path) -> io::Result<Self::Entries> {
        let entries = std::fs::read_dir(path)?;

        Ok(entries.map(directory_entry_path))
    }

    fn is_directory(&self, path: &Path) -> io::Result<bool> {
        std::fs::metadata(path).map(|metadata| metadata.is_dir())
    }

    fn is_file(&self, path: &Path) -> io::Result<bool> {
        std::fs::metadata(path).map(|metadata| metadata.is_file())
    }
}

/// Extracts a path from a directory entry while preserving entry errors.
fn directory_entry_path(entry: io::Result<std::fs::DirEntry>) -> io::Result<PathBuf> {
    entry.map(|entry| entry.path())
}

/// Finds feature files using the supplied directory-operation implementation.
fn find_feature_files_with<R: DirectoryReader>(
    workspace_root: &Path,
    reader: &R,
) -> Result<Vec<PathBuf>, ServerError> {
    let mut features = Vec::new();

    // Check common feature file locations
    let search_dirs = [
        workspace_root.join("tests").join("features"),
        workspace_root.join("features"),
    ];

    for dir in &search_dirs {
        collect_optional_feature_directory(reader, dir, &mut features)?;
    }

    // Also search in crate subdirectories
    search_crate_subdirectories(reader, workspace_root, &mut features)?;

    Ok(features)
}

/// Collects an optional feature directory, ignoring only a missing directory.
fn collect_optional_feature_directory<R: DirectoryReader>(
    reader: &R,
    directory: &Path,
    features: &mut Vec<PathBuf>,
) -> Result<(), ServerError> {
    match reader.is_directory(directory) {
        Ok(true) => collect_feature_files_recursive(reader, directory, features),
        Ok(false) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.into()),
    }
}

/// Checks an optional file path, ignoring only a missing file.
fn is_optional_file<R: DirectoryReader>(reader: &R, path: &Path) -> Result<bool, ServerError> {
    match reader.is_file(path) {
        Ok(is_file) => Ok(is_file),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error.into()),
    }
}

/// Search for feature files in crate subdirectories.
///
/// Looks for `tests/features/` directories within each subdirectory of the
/// workspace root (typical layout for multi-crate workspaces).
fn search_crate_subdirectories<R: DirectoryReader>(
    reader: &R,
    workspace_root: &Path,
    features: &mut Vec<PathBuf>,
) -> Result<(), ServerError> {
    let entries = reader.read_dir(workspace_root)?;

    for path in entries {
        let path = path?;
        if !reader.is_directory(&path)? {
            continue;
        }
        if !is_optional_file(reader, &path.join("Cargo.toml"))? {
            continue;
        }

        let crate_features = path.join("tests").join("features");
        collect_optional_feature_directory(reader, &crate_features, features)?;
    }

    Ok(())
}

/// Recursively collect `.feature` files from a directory.
fn collect_feature_files_recursive<R: DirectoryReader>(
    reader: &R,
    dir: &Path,
    features: &mut Vec<PathBuf>,
) -> Result<(), ServerError> {
    for path in reader.read_dir(dir)? {
        let path = path?;
        if reader.is_directory(&path)? {
            collect_feature_files_recursive(reader, &path, features)?;
        } else if path.extension().is_some_and(|ext| ext == "feature") {
            features.push(path);
        }
    }

    Ok(())
}

#[cfg(test)]
#[path = "workspace_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "workspace_observability_tests.rs"]
mod observability_tests;
