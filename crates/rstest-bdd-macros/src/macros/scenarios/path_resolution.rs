//! Canonicalises feature file paths for `scenarios!`, preferring capability-aware
//! resolution through `cap-std` before falling back to `std::fs` for escape
//! hatches.

use cap_std::ambient_authority;
use cap_std::fs::Dir;
use cap_std::AmbientAuthority;
use std::path::{Path, PathBuf};

fn canonicalize_absolute_path(
    path: &Path,
    authority: AmbientAuthority,
) -> std::io::Result<PathBuf> {
    let root = path
        .ancestors()
        .last()
        .unwrap_or_else(|| Path::new(std::path::MAIN_SEPARATOR_STR));
    let dir = Dir::open_ambient_dir(root, authority)?;
    let relative = path.strip_prefix(root).unwrap_or(path);
    let target = if relative.as_os_str().is_empty() {
        Path::new(".")
    } else {
        relative
    };
    let resolved = dir.canonicalize(target)?;
    if resolved.is_absolute() {
        Ok(resolved)
    } else {
        Ok(PathBuf::from(root).join(resolved))
    }
}

fn canonicalize_relative_path(
    path: &Path,
    authority: AmbientAuthority,
) -> std::io::Result<PathBuf> {
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let dir = Dir::open_ambient_dir(parent, authority)?;
    let target = if parent == Path::new(".") {
        path
    } else {
        path.strip_prefix(parent).unwrap_or(path)
    };
    let resolved = dir.canonicalize(target)?;
    if resolved.is_absolute() {
        Ok(resolved)
    } else if parent == Path::new(".") {
        Ok(std::env::current_dir()?.join(resolved))
    } else {
        Ok(parent.to_path_buf().join(resolved))
    }
}

fn try_std_canonicalize_fallback(path: &Path, err: std::io::Error) -> std::io::Result<PathBuf> {
    if err.kind() == std::io::ErrorKind::PermissionDenied
        && err.to_string().contains("outside of the filesystem")
    {
        // cap-std denies canonicalising absolute symlinks that escape a
        // capability root. Falling back to std ensures we still support such
        // links whilst preferring capability-aware resolution for every other
        // case.
        std::fs::canonicalize(path)
    } else {
        Err(err)
    }
}

pub(super) fn canonicalize_path(path: &Path) -> std::io::Result<PathBuf> {
    let authority = ambient_authority();
    let attempt = if path.is_absolute() {
        canonicalize_absolute_path(path, authority)
    } else {
        canonicalize_relative_path(path, authority)
    };

    match attempt {
        Ok(resolved) => Ok(resolved),
        Err(err) => try_std_canonicalize_fallback(path, err),
    }
}
