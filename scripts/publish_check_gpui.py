#!/usr/bin/env -S uv run python
"""Helpers for validating the packaged GPUI harness against upstream GPUI.

The main publish workflow keeps the repository on a stable-compatible GPUI shim
via a workspace patch. This module creates an isolated validator crate that
depends on the packaged `rstest-bdd-harness-gpui` artifact and the upstream
`gpui` crate from crates.io, allowing release automation to verify that the
published dependency graph still works without changing the main workspace.
"""

from __future__ import annotations

import pathlib
import shutil
import tarfile
import typing as typ

from publish_check_gpui_manifest import (
    _packaged_manifest,
    _validator_manifest,
    _validator_test_source,
)

if typ.TYPE_CHECKING:
    from pathlib import Path

GPUI_HARNESS_CRATE = "rstest-bdd-harness-gpui"
GPUI_VALIDATOR_CRATE = "rstest-bdd-harness-gpui-package-check"


def packaged_archive_path(workspace_root: Path, crate: str, version: str) -> Path:
    """Return the expected archive path produced by ``cargo package``.

    Parameters
    ----------
    workspace_root : Path
        Root directory of the exported workspace being validated.
    crate : str
        Name of the crate whose packaged archive path is being resolved.
    version : str
        Version string embedded in the packaged archive name.

    Returns
    -------
    Path
        Path to the expected ``.crate`` archive under ``target/package``.

    Examples
    --------
    >>> packaged_archive_path(pathlib.Path("/tmp/workspace"), "demo", "1.2.3")
    PosixPath('/tmp/workspace/target/package/demo-1.2.3.crate')
    """
    return workspace_root / "target" / "package" / f"{crate}-{version}.crate"


def build_packaged_archive(
    workspace_root: Path,
    destination: Path,
    version: str,
    *,
    timeout_secs: int | None = None,
) -> Path:
    """Create a standalone publish-shaped archive for the GPUI harness crate.

    Parameters
    ----------
    workspace_root : Path
        Root directory of the exported workspace being validated.
    destination : Path
        Archive path where the generated ``.crate`` file should be written.
    version : str
        Version string to embed in the packaged crate manifest and archive
        name.
    timeout_secs : int | None, optional
        Reserved for future subprocess-based packaging. The current synthetic
        packaging path does not use it.

    Returns
    -------
    Path
        Path to the generated standalone ``.crate`` archive.

    ``timeout_secs`` is reserved for future subprocess-based packaging and is
    currently unused because this helper synthesizes the archive directly.

    Examples
    --------
    >>> build_packaged_archive(  # doctest: +SKIP
    ...     pathlib.Path('/tmp/workspace'),
    ...     pathlib.Path(
    ...         '/tmp/workspace/target/package/rstest-bdd-harness-gpui-1.2.3.crate'
    ...     ),
    ...     '1.2.3',
    ... )
    PosixPath('/tmp/workspace/target/package/rstest-bdd-harness-gpui-1.2.3.crate')
    """
    _ = timeout_secs
    source_dir = workspace_root / "crates" / GPUI_HARNESS_CRATE
    package_root = destination.parent / f"{GPUI_HARNESS_CRATE}-{version}"
    if package_root.exists():
        shutil.rmtree(package_root)
    package_root.mkdir(parents=True, exist_ok=True)

    shutil.copytree(source_dir / "src", package_root / "src")
    shutil.copy2(source_dir / "README.md", package_root / "README.md")
    (package_root / "Cargo.toml").write_text(
        _packaged_manifest(workspace_root, version, GPUI_HARNESS_CRATE),
        encoding="utf-8",
    )

    destination.parent.mkdir(parents=True, exist_ok=True)
    with tarfile.open(destination, "w:gz") as package:
        for path in sorted(package_root.rglob("*")):
            if not path.is_file():
                continue
            relative = path.relative_to(package_root)
            package.add(path, arcname=f"{package_root.name}/{relative.as_posix()}")

    return destination


def _has_top_level_files(member_paths: list[pathlib.PurePosixPath]) -> bool:
    """Return ``True`` when any member sits directly at the archive root."""
    return any(len(member.parts) == 1 for member in member_paths)


def _find_root_names(
    archive: Path,
    member_paths: list[pathlib.PurePosixPath],
) -> tuple[str, set[str]]:
    """Return ``(root_name, all_root_names)`` or raise on an empty archive."""
    try:
        package_root_names = {member.parts[0] for member in member_paths}
        package_root_name = next(iter(package_root_names))
    except StopIteration as error:
        message = f"packaged archive {archive} did not contain any files"
        raise SystemExit(message) from error
    return package_root_name, package_root_names


def _resolve_archive_root(
    archive: Path, member_paths: list[pathlib.PurePosixPath]
) -> str:
    """Return the single top-level directory name from ``member_paths``.

    Raises ``SystemExit`` when the archive is empty, contains top-level file
    entries, or has more than one top-level directory.
    """
    package_root_name, package_root_names = _find_root_names(archive, member_paths)
    if _has_top_level_files(member_paths):
        message = f"packaged archive {archive} contained top-level file entries"
        raise SystemExit(message)
    if len(package_root_names) != 1:
        message = (
            f"packaged archive {archive} must contain exactly one top-level directory"
        )
        raise SystemExit(message)
    return package_root_name


def extract_packaged_archive(archive: Path, destination: Path) -> Path:
    """Extract ``archive`` into ``destination`` and return the package root.

    Parameters
    ----------
    archive : Path
        Packaged ``.crate`` archive to extract.
    destination : Path
        Directory where the archive contents should be unpacked.

    Returns
    -------
    Path
        Root directory of the extracted packaged crate.

    Examples
    --------
    >>> archive = pathlib.Path("/tmp/workspace/target/package/demo-1.2.3.crate")
    >>> destination = pathlib.Path("/tmp/unpacked")
    >>> extract_packaged_archive(archive, destination)  # doctest: +SKIP
    PosixPath('/tmp/unpacked/demo-1.2.3')
    """
    with tarfile.open(archive, "r:gz") as package:
        members = package.getmembers()
        member_paths = [
            pathlib.PurePosixPath(member.name.replace("\\", "/"))
            for member in members
            if member.name
        ]
        package_root_name = _resolve_archive_root(archive, member_paths)
        _extract_archive_safely(package, destination)

    return destination / package_root_name


def write_validator_workspace(
    destination: Path,
    *,
    package_dir: Path,
    harness_dir: Path,
    version: str,
) -> Path:
    """Create a minimal validator crate that targets upstream ``gpui``.

    Parameters
    ----------
    destination : Path
        Directory where the validator workspace should be created.
    package_dir : Path
        Extracted packaged GPUI harness crate directory.
    harness_dir : Path
        Exported local ``rstest-bdd-harness`` directory used for patching.
    version : str
        Workspace version to pin in the validator manifest.

    Returns
    -------
    Path
        Root directory of the generated validator workspace.

    Examples
    --------
    >>> write_validator_workspace(  # doctest: +SKIP
    ...     pathlib.Path('/tmp/validator'),
    ...     package_dir=pathlib.Path('/tmp/pkg/rstest-bdd-harness-gpui-1.2.3'),
    ...     harness_dir=pathlib.Path('/tmp/workspace/crates/rstest-bdd-harness'),
    ...     version='1.2.3',
    ... )
    PosixPath('/tmp/validator')
    """
    _reset_directory(destination)
    destination.mkdir(parents=True, exist_ok=True)
    tests_dir = destination / "tests"
    tests_dir.mkdir(exist_ok=True)
    (destination / "Cargo.toml").write_text(
        _validator_manifest(
            package_dir=package_dir,
            harness_dir=harness_dir,
            version=version,
            validator_crate=GPUI_VALIDATOR_CRATE,
        ),
        encoding="utf-8",
    )
    (tests_dir / "packaged_gpui_harness.rs").write_text(
        _validator_test_source(),
        encoding="utf-8",
    )
    return destination


def _is_link_member(member: tarfile.TarInfo) -> bool:
    """Return ``True`` when ``member`` is a symbolic or hard link."""
    return member.issym() or member.islnk()


def _is_allowed_member_type(member: tarfile.TarInfo) -> bool:
    """Return ``True`` when ``member`` is a regular file, directory, or link."""
    return member.isreg() or member.isdir() or _is_link_member(member)


def _assert_link_target_safe(
    resolved_destination: pathlib.Path,
    member: tarfile.TarInfo,
    member_destination: pathlib.Path,
) -> None:
    """Raise ``SystemExit`` if a link target would escape ``resolved_destination``."""
    if _is_link_member(member) and _is_unsafe_archive_path(
        resolved_destination,
        member.linkname,
        base_directory=member_destination.parent,
    ):
        message = f"refusing to extract unsafe archive member {member.name!r}"
        raise SystemExit(message)


def _assert_member_safe(
    resolved_destination: pathlib.Path, member: tarfile.TarInfo
) -> None:
    """Raise ``SystemExit`` if ``member`` would be unsafe to extract."""
    if not _is_allowed_member_type(member):
        message = f"refusing to extract unsupported archive member {member.name!r}"
        raise SystemExit(message)
    if _is_unsafe_archive_path(resolved_destination, member.name):
        message = f"refusing to extract unsafe archive member {member.name!r}"
        raise SystemExit(message)
    member_destination = _archive_target_path(resolved_destination, member.name)
    if member_destination is None:
        message = f"refusing to extract unsafe archive member {member.name!r}"
        raise SystemExit(message)
    _assert_link_target_safe(resolved_destination, member, member_destination)


def _extract_archive_safely(package: tarfile.TarFile, destination: Path) -> None:
    """Safely extract tar members into ``destination`` after validation."""
    resolved_destination = pathlib.Path(destination).resolve(strict=False)
    members = package.getmembers()
    for member in members:
        _assert_member_safe(resolved_destination, member)
    _reset_directory(destination)
    destination.mkdir(parents=True, exist_ok=True)
    for member in members:
        package.extract(member, destination)


def _reset_directory(destination: Path) -> None:
    """Remove an existing destination directory tree before repopulating it."""
    if not destination.exists():
        return
    if destination.is_dir():
        shutil.rmtree(destination)
        return
    message = f"refusing to reuse non-directory destination {destination!r}"
    raise SystemExit(message)


def _is_unsafe_archive_path(
    destination: pathlib.Path,
    path_name: str,
    *,
    base_directory: pathlib.Path | None = None,
) -> bool:
    """Return ``True`` when ``path_name`` would extract outside ``destination``."""
    target = _archive_target_path(base_directory or destination, path_name)
    return target is None or not _is_within_directory(destination, target)


def _is_rooted_path(
    posix_path: pathlib.PurePosixPath,
    windows_path: pathlib.PureWindowsPath,
) -> bool:
    """Return ``True`` when either path representation is absolute or drive-rooted."""
    return (
        posix_path.is_absolute()
        or windows_path.is_absolute()
        or bool(windows_path.drive)
    )


def _archive_target_path(
    base_directory: pathlib.Path, path_name: str
) -> pathlib.Path | None:
    """Resolve ``path_name`` under ``base_directory`` or return ``None`` if rooted."""
    posix_path = pathlib.PurePosixPath(path_name.replace("\\", "/"))
    windows_path = pathlib.PureWindowsPath(path_name)
    if _is_rooted_path(posix_path, windows_path):
        return None
    relative_parts = [part for part in posix_path.parts if part not in ("", ".")]
    return base_directory.joinpath(*relative_parts).resolve(strict=False)


def _is_within_directory(root: pathlib.Path, target: pathlib.Path) -> bool:
    """Return ``True`` when ``target`` is contained within ``root``."""
    return target == root or root in target.parents
