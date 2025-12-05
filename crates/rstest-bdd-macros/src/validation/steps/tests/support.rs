//! Shared helpers for step validation tests.

use super::*;
use camino::{Utf8Path, Utf8PathBuf};
use cap_std::{ambient_authority, fs_utf8::Dir};
use rstest::fixture;
use tempfile::tempdir;

pub(super) fn clear_registry() {
    #[expect(clippy::expect_used, reason = "registry lock must panic if poisoned")]
    REGISTERED.lock().expect("step registry poisoned").clear();
}

pub(super) fn create_test_step(keyword: StepKeyword, text: &str) -> ParsedStep {
    ParsedStep {
        keyword,
        text: text.to_string(),
        docstring: None,
        table: None,
        #[cfg(feature = "compile-time-validation")]
        span: proc_macro2::Span::call_site(),
    }
}

pub(super) fn assert_bullet_count(err: &str, expected: usize) {
    let bullet_count = err
        .lines()
        .filter(|l| l.trim_start().starts_with("- "))
        .count();
    assert_eq!(bullet_count, expected, "expected {expected} bullet matches");
}

pub(super) struct TempWorkingDir {
    _temp: tempfile::TempDir,
    path: Utf8PathBuf,
}

impl TempWorkingDir {
    fn new(temp: tempfile::TempDir, path: Utf8PathBuf) -> Self {
        Self { _temp: temp, path }
    }

    pub(super) fn path(&self) -> &Utf8Path {
        self.path.as_path()
    }

    pub(super) fn join(&self, relative: &str) -> Utf8PathBuf {
        self.path.join(relative)
    }
}

fn should_skip_creation(path: &Utf8Path) -> bool {
    path.as_str().is_empty() || path == Utf8Path::new(".")
}

fn ensure_parent_exists(path: &Utf8Path) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        if parent != path {
            create_dir_all_cap(parent)?;
        }
    }

    Ok(())
}

fn with_dir<T>(
    path: &Utf8Path,
    op: impl FnOnce(&Dir, &Utf8Path) -> std::io::Result<T>,
) -> std::io::Result<T> {
    let authority = ambient_authority();
    if let Some(parent) = path.parent() {
        if should_skip_creation(parent) {
            let dir = Dir::open_ambient_dir(Utf8Path::new("."), authority)?;
            let target = path.file_name().map_or(path, Utf8Path::new);
            return op(&dir, target);
        }

        let dir = Dir::open_ambient_dir(parent, authority)?;
        let target = path.file_name().map_or(path, Utf8Path::new);
        return op(&dir, target);
    }

    let dir = Dir::open_ambient_dir(Utf8Path::new("."), authority)?;
    op(&dir, path)
}

fn create_single_dir(path: &Utf8Path) -> std::io::Result<()> {
    with_dir(path, |dir, target| {
        dir.create_dir(target).or_else(|error| {
            if error.kind() == std::io::ErrorKind::AlreadyExists {
                Ok(())
            } else {
                Err(error)
            }
        })
    })
}

pub(super) fn create_dir_all_cap(path: &Utf8Path) -> std::io::Result<()> {
    if should_skip_creation(path) {
        return Ok(());
    }

    if path.file_name().is_none() {
        return ensure_parent_exists(path);
    }

    ensure_parent_exists(path)?;
    create_single_dir(path)?;

    Ok(())
}

fn temp_working_dir_inner() -> std::io::Result<TempWorkingDir> {
    let temp = tempdir()?;

    let temp_path = Utf8PathBuf::from_path_buf(temp.path().to_path_buf()).map_err(|_| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "temporary path should be valid UTF-8",
        )
    })?;
    // Ensure the directory is accessible; capability handle unused but validates creation.
    Dir::open_ambient_dir(&temp_path, ambient_authority())?;
    Ok(TempWorkingDir::new(temp, temp_path))
}

#[fixture]
pub(super) fn temp_working_dir() -> std::io::Result<TempWorkingDir> {
    temp_working_dir_inner()
}
