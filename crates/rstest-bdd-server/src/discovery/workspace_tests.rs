//! Unit tests for workspace discovery.

use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    io,
};

use rstest::{fixture, rstest};
use tempfile::TempDir;

use super::*;

/// Creates a valid temporary Cargo workspace for discovery tests.
#[rstest_bdd_test_macros::allow_fixture_expansion_lints]
#[fixture]
fn create_test_workspace() -> io::Result<TempDir> {
    let dir = TempDir::new()?;
    let cargo_toml = dir.path().join("Cargo.toml");
    fs::write(
        &cargo_toml,
        r#"[package]
name = "test-project"
version = "0.1.0"
edition = "2024"
"#,
    )?;

    // Create a simple src/lib.rs so the package is valid
    let src_dir = dir.path().join("src");
    fs::create_dir_all(&src_dir)?;
    fs::write(src_dir.join("lib.rs"), "")?;

    Ok(dir)
}

/// Discovers the workspace root and package when invoked from its root.
#[rstest]
fn discovers_workspace_from_root(create_test_workspace: io::Result<TempDir>) {
    let workspace = create_test_workspace.expect("test setup should succeed");
    let result = discover_workspace(workspace.path());
    assert!(result.is_ok());
    let info = result.expect("should discover workspace");
    assert_eq!(info.root, workspace.path());
    assert!(info.packages.contains(&"test-project".to_owned()));
}

/// Discovers the workspace root when invoked from a nested source directory.
#[rstest]
fn discovers_workspace_from_subdirectory(create_test_workspace: io::Result<TempDir>) {
    let workspace = create_test_workspace.expect("test setup should succeed");
    let subdir = workspace.path().join("src");
    let result = discover_workspace(&subdir);
    assert!(result.is_ok());
    let info = result.expect("should discover workspace");
    assert_eq!(info.root, workspace.path());
}

/// Reports that workspace discovery fails when no Cargo manifest is present.
#[rstest]
fn fails_when_no_manifest_found() {
    let dir = TempDir::new().expect("failed to create temp dir");
    let result = discover_workspace(dir.path());
    let err = result.expect_err("workspace discovery should fail when no Cargo.toml exists");
    assert!(err.to_string().contains("no Cargo.toml found"));
}

/// Creates a test workspace with a feature file in a specified directory.
///
/// # Arguments
///
/// * `relative_dir` - Path segments relative to the workspace root (e.g., `&["tests", "features"]`)
/// * `filename` - Name of the feature file to create
/// * `content` - Content to write to the feature file
///
/// # Returns
///
/// The result of calling `find_feature_files` on the workspace.
fn create_workspace_with_feature(
    relative_dir: &[&str],
    filename: &str,
    content: &str,
) -> Result<Vec<PathBuf>, ServerError> {
    let workspace = create_test_workspace()?;
    let mut dir = workspace.path().to_path_buf();
    for segment in relative_dir {
        dir = dir.join(segment);
    }
    fs::create_dir_all(&dir)?;
    fs::write(dir.join(filename), content)?;

    find_feature_files(workspace.path())
}

/// Discovers feature files in supported directories, including nested paths.
#[rstest]
#[case(&["tests", "features"], "example.feature", "Feature: Test")]
#[case(&["tests", "features", "nested"], "nested.feature", "Feature: Nested")]
fn finds_feature_files_in_various_locations(
    #[case] relative_dir: &[&str],
    #[case] filename: &str,
    #[case] content: &str,
) {
    let features = create_workspace_with_feature(relative_dir, filename, content)
        .expect("test setup and feature discovery should succeed");

    assert_eq!(features.len(), 1);
    assert!(
        features
            .first()
            .expect("should have one feature")
            .ends_with(filename)
    );
}

/// Returns no feature files when the workspace contains none.
#[rstest]
fn returns_empty_when_no_feature_files(create_test_workspace: io::Result<TempDir>) {
    let workspace = create_test_workspace.expect("test setup should succeed");
    let features = find_feature_files(workspace.path()).expect("feature discovery should succeed");
    assert!(features.is_empty());
}

/// Creates and removes a temporary directory, returning its missing path.
fn create_missing_path() -> PathBuf {
    let temporary_directory = match TempDir::new() {
        Ok(temporary_directory) => temporary_directory,
        Err(error) => panic!("failed to create temp dir: {error}"),
    };
    let missing_path = temporary_directory.path().to_path_buf();
    if let Err(error) = temporary_directory.close() {
        panic!("failed to remove temp dir: {error}");
    }

    missing_path
}

/// Asserts that an error is `ServerError::Io` with `io::ErrorKind::NotFound`.
fn assert_not_found_error(error: ServerError) {
    assert_io_error_kind(error, io::ErrorKind::NotFound);
}

/// Asserts that a `ServerError::Io` wraps the expected I/O error kind.
pub(super) fn assert_io_error_kind(error: ServerError, expected_kind: io::ErrorKind) {
    let ServerError::Io(source) = error else {
        panic!("expected an I/O error");
    };

    assert_eq!(source.kind(), expected_kind);
}

/// Selects a deterministic operation in the in-memory workspace model.
#[derive(Clone, Copy, Eq, PartialEq)]
pub(super) enum FailureSite {
    /// Fails the required workspace-root directory read.
    WorkspaceReadDirectory,
    /// Fails required workspace-root directory-entry iteration.
    WorkspaceDirectoryEntry,
    /// Fails a required recursive feature-directory read.
    RecursiveDirectoryRead,
    /// Fails required entry iteration in a recursive feature directory.
    RecursiveDirectoryEntry,
    /// Fails required crate-directory metadata.
    CrateDirectoryMetadata,
    /// Fails required crate-manifest metadata.
    CrateManifestMetadata,
    /// Fails the optional `tests/features` metadata probe.
    OptionalTestsFeaturesMetadata,
    /// Fails the optional workspace `features` metadata probe.
    OptionalWorkspaceFeaturesMetadata,
    /// Fails an optional crate-manifest metadata probe.
    OptionalCrateManifestMetadata,
}
/// Nested feature directory reached by every generated property-test workspace.
const RECURSIVE_FAILURE_DIRECTORY: &str = "workspace/crate/tests/features/nested-0-0";

/// Supplies a bounded in-memory filesystem model for discovery properties.
pub(super) struct InMemoryDirectoryReader {
    entries: BTreeMap<PathBuf, Vec<PathBuf>>,
    directories: BTreeSet<PathBuf>,
    files: BTreeSet<PathBuf>,
    failure: Option<(FailureSite, io::ErrorKind)>,
}
impl DirectoryReader for InMemoryDirectoryReader {
    type Entries = std::vec::IntoIter<io::Result<PathBuf>>;
    /// Reads modelled entries or returns the configured directory-read failure.
    fn read_dir(&self, path: &Path) -> io::Result<Self::Entries> {
        match self.failure {
            Some((FailureSite::WorkspaceReadDirectory, kind)) if path == Path::new("workspace") => {
                return Err(io::Error::from(kind));
            }
            Some((FailureSite::RecursiveDirectoryRead, kind))
                if path == Path::new(RECURSIVE_FAILURE_DIRECTORY) =>
            {
                return Err(io::Error::from(kind));
            }
            _ => {}
        }
        let mut entries = self.entries.get(path).cloned().unwrap_or_default();
        let entry_failure_kind = match self.failure {
            Some((FailureSite::WorkspaceDirectoryEntry, kind))
                if path == Path::new("workspace") =>
            {
                Some(kind)
            }
            Some((FailureSite::RecursiveDirectoryEntry, kind))
                if path == Path::new(RECURSIVE_FAILURE_DIRECTORY) =>
            {
                Some(kind)
            }
            _ => None,
        };
        if entry_failure_kind.is_some() {
            entries.insert(0, PathBuf::new());
        }
        Ok(entries
            .into_iter()
            .map(move |entry| {
                if entry.as_os_str().is_empty()
                    && let Some(kind) = entry_failure_kind
                {
                    return Err(io::Error::from(kind));
                }
                Ok(entry)
            })
            .collect::<Vec<_>>()
            .into_iter())
    }
    /// Reports modelled directory metadata or the configured metadata failure.
    fn is_directory(&self, path: &Path) -> io::Result<bool> {
        if self.directory_metadata_failure(path) {
            let Some((_, kind)) = self.failure else {
                return Ok(self.directories.contains(path));
            };
            return Err(io::Error::from(kind));
        }

        Ok(self.directories.contains(path))
    }
    /// Reports modelled file metadata or the configured manifest-probe failure.
    fn is_file(&self, path: &Path) -> io::Result<bool> {
        if self.metadata_failure(FailureSite::CrateManifestMetadata, path)
            || self.metadata_failure(FailureSite::OptionalCrateManifestMetadata, path)
        {
            let Some((_, kind)) = self.failure else {
                return Ok(self.files.contains(path));
            };
            return Err(io::Error::from(kind));
        }

        Ok(self.files.contains(path))
    }
}

impl InMemoryDirectoryReader {
    /// Checks whether a required or optional directory metadata probe must fail.
    fn directory_metadata_failure(&self, path: &Path) -> bool {
        self.metadata_failure(FailureSite::CrateDirectoryMetadata, path)
            || self.optional_directory_metadata_failure(path)
    }

    /// Checks whether an optional feature-directory metadata probe must fail.
    fn optional_directory_metadata_failure(&self, path: &Path) -> bool {
        self.metadata_failure(FailureSite::OptionalTestsFeaturesMetadata, path)
            || self.metadata_failure(FailureSite::OptionalWorkspaceFeaturesMetadata, path)
    }

    /// Checks whether the configured metadata failure applies to a path.
    fn metadata_failure(&self, site: FailureSite, path: &Path) -> bool {
        let Some((configured_site, _)) = self.failure else {
            return false;
        };

        configured_site == site
            && match site {
                FailureSite::CrateDirectoryMetadata => path == Path::new("workspace/crate"),
                FailureSite::CrateManifestMetadata => {
                    path == Path::new("workspace/crate/Cargo.toml")
                }
                FailureSite::OptionalTestsFeaturesMetadata => {
                    path == Path::new("workspace/tests/features")
                }
                FailureSite::OptionalWorkspaceFeaturesMetadata => {
                    path == Path::new("workspace/features")
                }
                FailureSite::OptionalCrateManifestMetadata => {
                    path == Path::new("workspace/optional-crate/Cargo.toml")
                }
                FailureSite::WorkspaceReadDirectory
                | FailureSite::WorkspaceDirectoryEntry
                | FailureSite::RecursiveDirectoryRead
                | FailureSite::RecursiveDirectoryEntry => false,
            }
    }
}

/// Holds a model reader and the feature paths that successful discovery must return.
pub(super) struct InMemoryWorkspace {
    /// Reader implementing the generated workspace tree and injected failure.
    pub(super) reader: InMemoryDirectoryReader,
    /// Sorted feature paths expected from successful discovery.
    pub(super) expected_features: Vec<PathBuf>,
}

/// Builds a bounded workspace tree with nested feature leaves and ordered entries.
pub(super) fn in_memory_workspace(
    nested_feature_depths: &[u8],
    entry_order: &[u8],
    failure: Option<(FailureSite, io::ErrorKind)>,
) -> InMemoryWorkspace {
    let workspace = PathBuf::from("workspace");
    let crate_root = workspace.join("crate");
    let optional_crate = workspace.join("optional-crate");
    let feature_root = crate_root.join("tests/features");
    let mut entries = BTreeMap::new();
    let mut directories = BTreeSet::from([
        workspace.clone(),
        crate_root.clone(),
        optional_crate.clone(),
        crate_root.join("tests"),
        feature_root.clone(),
    ]);
    let files = BTreeSet::from([crate_root.join("Cargo.toml")]);
    let mut expected_features = Vec::new();

    entries.insert(workspace.clone(), vec![crate_root.clone(), optional_crate]);
    for (feature_index, depth) in nested_feature_depths.iter().enumerate() {
        let mut directory = feature_root.clone();
        for level in 0..*depth {
            let nested = directory.join(format!("nested-{feature_index}-{level}"));
            entries
                .entry(directory.clone())
                .or_default()
                .push(nested.clone());
            directories.insert(nested.clone());
            directory = nested;
        }
        let feature = directory.join(format!("feature-{feature_index}.feature"));
        entries.entry(directory).or_default().push(feature.clone());
        expected_features.push(feature);
    }

    for directory_entries in entries.values_mut() {
        order_entries(directory_entries, entry_order);
    }
    expected_features.sort();

    InMemoryWorkspace {
        reader: InMemoryDirectoryReader {
            entries,
            directories,
            files,
            failure,
        },
        expected_features,
    }
}

/// Applies generated ordering keys while preserving a deterministic path tie-breaker.
fn order_entries(entries: &mut [PathBuf], entry_order: &[u8]) {
    if entry_order.is_empty() {
        return;
    }

    let mut indexed_entries = entries.iter().cloned().enumerate().collect::<Vec<_>>();
    indexed_entries.sort_by_key(|(index, path)| {
        (
            entry_order.get(*index).copied().unwrap_or_default(),
            path.clone(),
        )
    });
    for (entry, (_, path)) in entries.iter_mut().zip(indexed_entries) {
        *entry = path;
    }
}

/// Returns `ServerError::Io` with `io::ErrorKind::NotFound` for a missing workspace.
#[test]
fn reports_workspace_read_failure() {
    let missing_path = create_missing_path();

    let error = find_feature_files(&missing_path)
        .expect_err("missing workspace should return an I/O error");

    assert_not_found_error(error);
}

/// Returns `ServerError::Io` with `io::ErrorKind::NotFound` for a missing feature directory.
#[test]
fn reports_recursive_directory_read_failure() {
    let missing_path = create_missing_path();
    let mut features = Vec::new();

    let error =
        collect_feature_files_recursive(&StandardDirectoryReader, &missing_path, &mut features)
            .expect_err("missing feature directory should return an I/O error");

    assert_not_found_error(error);
}
