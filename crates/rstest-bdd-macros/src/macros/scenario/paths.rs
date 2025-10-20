use cap_std::{ambient_authority, fs::Dir};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{LazyLock, RwLock};

/// Cache of canonicalised feature paths to avoid repeated filesystem lookups.
static FEATURE_PATH_CACHE: LazyLock<RwLock<HashMap<PathBuf, String>>> =
    LazyLock::new(|| RwLock::new(HashMap::new()));

/// Normalise path components so equivalent inputs share cache entries.
///
/// Policy:
/// - Do not alter absolute or prefixed paths; leave absolute resolution to filesystem canonicalisation.
/// - Collapse internal `.` segments.
/// - Collapse `..` only when a prior non-`..` segment exists; otherwise preserve leading `..`.
fn normalise(path: &Path) -> PathBuf {
    use std::ffi::OsString;
    use std::path::Component;

    if path.is_absolute() {
        return path.to_path_buf();
    }

    let mut segs: Vec<OsString> = Vec::new();
    for c in path.components() {
        match c {
            Component::CurDir => {}
            Component::ParentDir => {
                if segs.last().is_some_and(|s| s != "..") {
                    segs.pop();
                } else {
                    segs.push(OsString::from(".."));
                }
            }
            Component::Normal(s) => segs.push(s.to_os_string()),
            _ => segs.push(c.as_os_str().to_os_string()),
        }
    }
    let mut out = PathBuf::new();
    for s in segs {
        out.push(s);
    }
    out
}

#[cfg(all(test, windows))]
mod windows_paths {
    use super::normalise;
    use std::path::Path;

    #[test]
    fn preserves_drive_relative_parent_segments() {
        let p = Path::new(r"C:foo\..\bar");
        assert_eq!(normalise(p).to_string_lossy(), r"C:bar");
    }

    #[test]
    fn does_not_mangle_unc_prefix() {
        let p = Path::new(r"\\server\share\.\dir\..\file");
        assert_eq!(normalise(p), p);
    }
}

fn canonicalise_with_cap_std(path: &Path) -> Option<PathBuf> {
    let authority = ambient_authority();
    if path.is_absolute() {
        let Some(parent) = path.parent() else {
            return Some(path.to_path_buf());
        };
        let Some(name) = path.file_name() else {
            return Some(path.to_path_buf());
        };
        let name = PathBuf::from(name);
        let dir = Dir::open_ambient_dir(parent, authority).ok()?;
        let resolved = dir.canonicalize(&name).ok()?;
        Some(parent.to_path_buf().join(resolved))
    } else {
        let cwd = std::env::current_dir().ok()?;
        let dir = Dir::open_ambient_dir(&cwd, authority).ok()?;
        let resolved = dir.canonicalize(path).ok()?;
        Some(cwd.join(resolved))
    }
}

/// Canonicalise the feature path for stable diagnostics.
///
/// Resolves symlinks via cap-std directory canonicalisation so diagnostics
/// and generated code reference a consistent absolute path across builds.
/// The returned `String` is produced with [`Path::display`], so non-UTF-8
/// components are lossy.
///
/// # Examples
///
/// ```rust,ignore
/// use std::path::{Path, PathBuf};
///
/// let path = PathBuf::from("features/example.feature");
/// let _ = canonical_feature_path(&path);
/// ```
pub(super) fn canonical_feature_path(path: &Path) -> String {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").ok().map(PathBuf::from);
    // Scope cache keys by manifest dir to avoid cross-crate collisions.
    let key = if path.is_absolute() {
        normalise(path)
    } else if let Some(ref dir) = manifest_dir {
        dir.join(normalise(path))
    } else {
        normalise(path)
    };

    if let Some(cached) = {
        let cache = FEATURE_PATH_CACHE
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        cache.get(&key).cloned()
    } {
        return cached;
    }

    let canonical = manifest_dir
        .as_ref()
        .map(|d| d.join(path))
        .and_then(|p| canonicalise_with_cap_std(&p))
        .unwrap_or_else(|| PathBuf::from(path))
        .display()
        .to_string();

    let mut cache = FEATURE_PATH_CACHE
        .write()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let entry = cache.entry(key).or_insert_with(|| canonical.clone());
    entry.clone()
}

#[cfg(test)]
fn clear_feature_path_cache() {
    FEATURE_PATH_CACHE
        .write()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .clear();
}

#[cfg(test)]
mod tests {
    use super::{canonical_feature_path, canonicalise_with_cap_std, clear_feature_path_cache};
    use rstest::{fixture, rstest};
    use serial_test::serial;
    use std::env;
    use std::path::{Path, PathBuf};

    #[fixture]
    fn cache_cleared() {
        clear_feature_path_cache();
    }

    fn dir_and_target(path: &Path) -> std::io::Result<(super::Dir, PathBuf)> {
        let authority = super::ambient_authority();
        if path.is_absolute() {
            let parent = path.parent().ok_or_else(|| {
                std::io::Error::new(std::io::ErrorKind::InvalidInput, "path missing parent")
            })?;
            let file_name = path.file_name().ok_or_else(|| {
                std::io::Error::new(std::io::ErrorKind::InvalidInput, "path missing file name")
            })?;
            let dir = super::Dir::open_ambient_dir(parent, authority)?;
            return Ok((dir, PathBuf::from(file_name)));
        }

        let cwd = std::env::current_dir()?;
        let dir = super::Dir::open_ambient_dir(&cwd, authority)?;
        Ok((dir, path.into()))
    }

    fn create_dir_all_cap(path: &Path) -> std::io::Result<()> {
        if path.as_os_str().is_empty() || path == Path::new(".") {
            return Ok(());
        }

        if path.is_absolute() {
            let Some(parent) = path.parent() else {
                return Ok(());
            };
            if parent != path {
                create_dir_all_cap(parent)?;
            }
        }

        let (dir, target) = dir_and_target(path)?;
        match dir.create_dir_all(&target) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => Ok(()),
            Err(error) => Err(error),
        }
    }

    fn write_file_cap(path: &Path, contents: &[u8]) -> std::io::Result<()> {
        if path.is_absolute() {
            let Some(parent) = path.parent() else {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    "path missing parent",
                ));
            };
            create_dir_all_cap(parent)?;
        }

        let (dir, target) = dir_and_target(path)?;
        dir.write(&target, contents)
    }

    fn remove_file_cap(path: &Path) -> std::io::Result<()> {
        let (dir, target) = dir_and_target(path)?;
        dir.remove_file(&target)
    }

    #[serial]
    #[rstest]
    #[expect(
        clippy::expect_used,
        reason = "tests require explicit failure messages"
    )]
    fn canonicalises_with_manifest_dir(_cache_cleared: ()) {
        let manifest = PathBuf::from(
            env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR is required for tests"),
        );
        let path = Path::new("Cargo.toml");
        let expected = canonicalise_with_cap_std(&manifest.join(path))
            .expect("canonical path")
            .display()
            .to_string();
        assert_eq!(canonical_feature_path(path), expected);
    }

    #[serial]
    #[rstest]
    fn falls_back_on_missing_path(_cache_cleared: ()) {
        let path = Path::new("does-not-exist.feature");
        assert_eq!(canonical_feature_path(path), path.display().to_string());
    }

    #[serial]
    #[rstest]
    fn equivalent_relatives_map_to_same_result(_cache_cleared: ()) {
        let a = Path::new("./features/../features/example.feature");
        let b = Path::new("features/example.feature");
        assert_eq!(canonical_feature_path(a), canonical_feature_path(b));
    }

    #[serial]
    #[rstest]
    #[expect(
        clippy::expect_used,
        reason = "tests require explicit failure messages"
    )]
    fn caches_paths_between_calls(_cache_cleared: ()) {
        use std::time::{SystemTime, UNIX_EPOCH};

        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let file_name = format!("cache_{unique}.feature");
        let manifest = PathBuf::from(
            env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR is required for tests"),
        );
        let tmp_dir = manifest.join("target/canonical-path-cache-tests");
        create_dir_all_cap(&tmp_dir).expect("create tmp dir");
        let file_path = tmp_dir.join(&file_name);
        write_file_cap(&file_path, b"").expect("create temp feature file");

        let rel_path = format!("target/canonical-path-cache-tests/{file_name}");
        let path = Path::new(&rel_path);
        let first = canonical_feature_path(path);

        remove_file_cap(&file_path).expect("remove temp feature file");
        let second = canonical_feature_path(path);

        assert_eq!(first, second);
    }
}
