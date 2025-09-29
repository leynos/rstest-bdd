"""Archive helpers for exporting publishable workspace snapshots.

The functions defined here encapsulate the tarball export logic used by the
publish automation. They deliberately keep filesystem side effects scoped to the
provided destination so callers can stage archives without mutating the working
copy.

Example
-------
>>> from pathlib import Path
>>> export_workspace(Path("/tmp/release-staging"))
"""

from __future__ import annotations

import sys
import tarfile
import tempfile
from pathlib import Path

from plumbum import local
from plumbum.commands import CommandNotFound

PROJECT_ROOT = Path(__file__).resolve().parents[1]


__all__ = ["export_workspace"]


GIT_ARCHIVE_TIMEOUT_S = 60


def _validated_members(
    tar: tarfile.TarFile, destination: Path
) -> list[tarfile.TarInfo]:
    """Validate entries against destination and return a safe subset."""
    safe_root = Path(destination).resolve()
    # Only allow regular files, directories, and link entries on older Pythons.
    allowed = {
        tarfile.REGTYPE,
        tarfile.AREGTYPE,
        tarfile.DIRTYPE,
        tarfile.SYMTYPE,
        tarfile.LNKTYPE,
    }
    members: list[tarfile.TarInfo] = []
    for member in tar.getmembers():
        if member.type not in allowed:
            detail = repr(member.name)
            message = f"refusing to extract unsupported tar entry type: {detail}"
            raise SystemExit(message)
        candidate_path = (safe_root / member.name).resolve()
        _ensure_member_within_destination(candidate_path, safe_root, member)
        if member.islnk() or member.issym():
            _ensure_link_within_destination(candidate_path, safe_root, member)
        members.append(member)
    return members


def _ensure_member_within_destination(
    candidate_path: Path, safe_root: Path, member: tarfile.TarInfo
) -> None:
    """Abort when ``member`` would escape ``safe_root`` during extraction."""
    message = f"refusing to extract member outside destination: {member.name!r}"
    _assert_within_destination(candidate_path, safe_root, message)


def _ensure_link_within_destination(
    candidate_path: Path, safe_root: Path, member: tarfile.TarInfo
) -> None:
    """Abort when link targets escape ``safe_root`` during extraction."""
    if member.islnk():
        target_path = safe_root.joinpath(member.linkname).resolve(strict=False)
    else:
        target_path = _resolve_link_target(candidate_path, member.linkname)
    detail = repr(member.name)
    message = "refusing to extract link entry outside destination: " + detail
    _assert_within_destination(target_path, safe_root, message)


def _resolve_link_target(candidate_path: Path, linkname: str) -> Path:
    """Return the absolute path a link would resolve to when extracted."""
    link_target = Path(linkname)
    if link_target.is_absolute():
        return link_target.resolve()
    return (candidate_path.parent / link_target).resolve()


def _assert_within_destination(path: Path, safe_root: Path, message: str) -> None:
    try:
        path.relative_to(safe_root)
    except ValueError as error:  # pragma: no cover - defensive branch
        raise SystemExit(message) from error


def export_workspace(destination: Path) -> None:
    """Extract the repository HEAD into ``destination`` via ``git archive``.

    Parameters
    ----------
    destination : Path
        Directory that will receive the exported workspace contents.

    Returns
    -------
    None
        The repository snapshot is written directly to ``destination`` and the
        export honours ``GIT_ARCHIVE_TIMEOUT_S`` to bound ``git`` execution.
    """
    destination = Path(destination)
    destination.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory() as archive_dir:
        archive_root = Path(archive_dir)
        archive_path = _create_archive(archive_root)
        _extract_archive(archive_path, destination)


def _create_archive(archive_root: Path) -> Path:
    """Run ``git archive`` and return the resulting tarball path."""
    archive_path = archive_root / "workspace.tar"
    with local.cwd(PROJECT_ROOT):
        try:
            git_archive = local["git"][
                "archive", "--format=tar", "HEAD", f"--output={archive_path}"
            ]
            return_code, stdout, stderr = git_archive.run(
                timeout=GIT_ARCHIVE_TIMEOUT_S,
                retcode=None,
            )
        except CommandNotFound as error:
            message = "git not found on PATH; unable to export workspace"
            raise SystemExit(message) from error
    if return_code != 0:
        diagnostics = (stderr or stdout or "").strip()
        detail = f": {diagnostics}" if diagnostics else ""
        message = f"git archive failed with exit code {return_code}{detail}"
        raise SystemExit(message)
    return archive_path


def _extract_archive(archive_path: Path, destination: Path) -> None:
    """Extract ``archive_path`` into ``destination`` after validation."""
    if not archive_path.exists():
        message = f"archive not found at {archive_path!s}"
        raise SystemExit(message)
    with tarfile.open(archive_path) as tar:
        safe_members = _validated_members(tar, destination)
        _extract_members(tar, destination, safe_members)


def _extract_members(
    tar: tarfile.TarFile, destination: Path, safe_members: list[tarfile.TarInfo]
) -> None:
    """Extract ``safe_members`` into ``destination`` with version-aware safety."""
    extract_kwargs = {}
    if sys.version_info >= (3, 12):
        extract_kwargs["filter"] = "data"
    for member in safe_members:
        tar.extract(member, destination, **extract_kwargs)
