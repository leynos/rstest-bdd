"""Archive helpers for exporting publishable workspace snapshots.

The functions defined here encapsulate the tarball export logic used by the
publish automation. They deliberately keep filesystem side effects scoped to the
provided destination so callers can stage archives without mutating the working
copy.
"""

from __future__ import annotations

import sys
import tarfile
import tempfile
from pathlib import Path

from plumbum import local

PROJECT_ROOT = Path(__file__).resolve().parents[1]


__all__ = ["export_workspace"]


def _validated_members(
    tar: tarfile.TarFile, destination: Path
) -> list[tarfile.TarInfo]:
    safe_root = Path(destination).resolve()
    members: list[tarfile.TarInfo] = []
    for member in tar.getmembers():
        candidate_path = (safe_root / member.name).resolve()
        try:
            candidate_path.relative_to(safe_root)
        except ValueError as error:  # pragma: no cover - defensive branch
            message = f"refusing to extract member outside destination: {member.name!r}"
            raise SystemExit(message) from error
        if member.islnk() or member.issym():
            link_target = Path(member.linkname)
            if link_target.is_absolute():
                target_path = Path(link_target).resolve()
            else:
                target_path = (candidate_path.parent / link_target).resolve()
            try:
                target_path.relative_to(safe_root)
            except ValueError as error:  # pragma: no cover - defensive branch
                detail = repr(member.name)
                message = (
                    "refusing to extract link entry outside destination: "
                    + detail
                )
                raise SystemExit(message) from error
        members.append(member)
    return members


def export_workspace(destination: Path) -> None:
    """Extract the repository HEAD into ``destination`` via ``git archive``.

    Parameters
    ----------
    destination : Path
        Directory that will receive the exported workspace contents.

    Returns
    -------
    None
        The repository snapshot is written directly to ``destination``.
    """
    with tempfile.TemporaryDirectory() as archive_dir:
        archive_path = Path(archive_dir) / "workspace.tar"
        git_archive = local["git"][
            "archive", "--format=tar", "HEAD", f"--output={archive_path}"
        ]
        with local.cwd(PROJECT_ROOT):
            return_code, stdout, stderr = git_archive.run(
                timeout=60,
                retcode=None,
            )
        if return_code != 0:
            diagnostics = (stderr or stdout or "").strip()
            detail = f": {diagnostics}" if diagnostics else ""
            message = f"git archive failed with exit code {return_code}{detail}"
            raise SystemExit(message)
        with tarfile.open(archive_path) as tar:
            safe_members = _validated_members(tar, destination)
            if sys.version_info >= (3, 12):
                for member in safe_members:
                    tar.extract(member, destination, filter="data")
            else:
                for member in safe_members:
                    tar.extract(member, destination)
