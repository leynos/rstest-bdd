//! File staging utilities for trybuild support.
//!
//! Handles discovery and copying of feature files to the trybuild test
//! environment, ensuring fixtures can locate their dependencies at compile time.

use camino::{Utf8Path, Utf8PathBuf};
use cap_std::{ambient_authority, fs::Dir};
use std::env;
use std::io;
use std::path::Path as StdPath;

const MACROS_FIXTURES_DIR: &str = "tests/fixtures_macros";
const FEATURES_DIR: &str = "tests/features";

pub(crate) fn macros_fixture(case: &str) -> Utf8PathBuf {
    ensure_trybuild_support_files();
    Utf8PathBuf::from(MACROS_FIXTURES_DIR).join(case)
}

pub(crate) fn ui_fixture(case: &str) -> Utf8PathBuf {
    Utf8PathBuf::from("tests/ui_macros").join(case)
}

fn ensure_trybuild_support_files() {
    use std::sync::OnceLock;
    static TRYBUILD_SUPPORT: OnceLock<()> = OnceLock::new();
    TRYBUILD_SUPPORT.get_or_init(|| {
        stage_trybuild_support_files().unwrap_or_else(|error| {
            panic!("failed to stage trybuild support files: {error}");
        });
    });
}

fn stage_trybuild_support_files() -> io::Result<()> {
    let crate_root = Utf8Path::new(env!("CARGO_MANIFEST_DIR"));
    let workspace_root = crate_root
        .parent()
        .and_then(Utf8Path::parent)
        .map(Utf8Path::to_owned)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "workspace root must exist"))?;
    let workspace_dir = Dir::open_ambient_dir(workspace_root.as_std_path(), ambient_authority())?;

    let target_tests_relative = Utf8Path::new("target/tests/trybuild");
    let trybuild_crate_relative = target_tests_relative.join("rstest-bdd");
    let workspace_features_relative = target_tests_relative.join("features");

    remove_dir_if_exists(&workspace_dir, workspace_features_relative.as_path())?;
    remove_dir_if_exists(&workspace_dir, trybuild_crate_relative.as_path())?;

    workspace_dir.create_dir_all(workspace_features_relative.as_std_path())?;
    workspace_dir.create_dir_all(trybuild_crate_relative.as_std_path())?;

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
        &workspace_dir,
        workspace_features_relative.as_std_path(),
        &features,
    )?;
    write_feature_files(
        &workspace_dir,
        trybuild_crate_relative.as_std_path(),
        &fixture_features,
    )?;

    // Stage auto-discovery feature files for `scenarios!` compile-pass test.
    // Derive auto features as a subset of the main features list to avoid
    // re-walking the filesystem.
    let auto_features = features
        .into_iter()
        .filter(|(path, _)| path.starts_with("auto/"))
        .collect::<Vec<_>>();

    if !auto_features.is_empty() {
        let auto_dest = trybuild_crate_relative.join("tests/features/auto");
        workspace_dir.create_dir_all(auto_dest.as_std_path())?;
        write_feature_files(&workspace_dir, auto_dest.as_std_path(), &auto_features)?;
    }

    Ok(())
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
        let file_name = entry.file_name().to_string_lossy().into_owned();
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
