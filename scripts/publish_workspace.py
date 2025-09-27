"""Workspace preparation helpers shared by publish automation scripts.

The routines in this module manipulate an exported copy of the repository so
release workflows can operate on a clean tree. They are intentionally
side-effect free apart from file-system writes inside the provided workspace
root, which keeps the publish scripts deterministic and safe to run locally.
"""

from __future__ import annotations

import tarfile
import tempfile
import typing as typ
from pathlib import Path

import tomllib
from plumbum import local
from publish_patch import REPLACEMENTS, apply_replacements
from tomlkit import array, dumps, parse

PROJECT_ROOT = Path(__file__).resolve().parents[1]

PUBLISHABLE_CRATES: typ.Final[tuple[str, ...]] = (
    "rstest-bdd-patterns",
    "rstest-bdd-macros",
    "rstest-bdd",
    "cargo-bdd",
)


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
            tar.extractall(destination, filter="data")


def _is_patch_section_start(line: str) -> bool:
    """Return True when the line marks the ``[patch.crates-io]`` section."""
    canonical = line.split("#", 1)[0].strip()
    return canonical == "[patch.crates-io]"


def _is_any_section_start(line: str) -> bool:
    """Return True when the line starts a new manifest section."""
    return line.lstrip().startswith("[")


def _process_patch_section_line(
    line: str, *, skipping_patch: bool
) -> tuple[bool, bool]:
    """Process manifest lines while tracking patch section boundaries.

    Parameters
    ----------
    line : str
        The manifest line currently being inspected.
    skipping_patch : bool
        ``True`` when the caller is currently omitting patch-section lines.

    Returns
    -------
    tuple[bool, bool]
        A tuple of ``(should_include_line, new_skipping_patch_state)`` where
        inline ``#`` comments have been stripped before the section checks run.
    """
    marker = line.split("#", 1)[0]

    if not skipping_patch and _is_patch_section_start(marker):
        return False, True

    if skipping_patch and _is_any_section_start(marker):
        return True, False

    return not skipping_patch, skipping_patch


def _ensure_proper_file_ending(lines: list[str]) -> None:
    """Ensure the file ends with a newline by adding an empty string if needed."""
    if not lines or lines[-1] != "":
        lines.append("")


def _rewrite_manifest_lines(
    manifest: Path, processor: typ.Callable[[list[str]], list[str]]
) -> None:
    """Read ``manifest`` lines, transform them, and write the updated content."""
    lines = manifest.read_text(encoding="utf-8").splitlines()
    rewritten = processor(lines)
    manifest.write_text("\n".join(rewritten), encoding="utf-8")


def strip_patch_section(manifest: Path) -> None:
    """Strip the ``[patch.crates-io]`` section from ``manifest``.

    Parameters
    ----------
    manifest : Path
        Manifest whose patch entries should be removed in place.

    Returns
    -------
    None
        The manifest on disk is rewritten without the patch section.
    """

    def _remove_patch(lines: list[str]) -> list[str]:
        cleaned: list[str] = []
        skipping_patch = False

        for line in lines:
            should_include, skipping_patch = _process_patch_section_line(
                line, skipping_patch=skipping_patch
            )
            if should_include:
                cleaned.append(line)

        _ensure_proper_file_ending(cleaned)
        return cleaned

    _rewrite_manifest_lines(manifest, _remove_patch)


def prune_workspace_members(manifest: Path) -> None:
    """Remove non-crate entries from the workspace members list.

    Parameters
    ----------
    manifest : Path
        Workspace manifest whose members array should only contain crates.

    Returns
    -------
    None
        The manifest is rewritten with only crate directories listed.
    """
    document = parse(manifest.read_text(encoding="utf-8"))
    workspace = document.get("workspace")
    if workspace is None:
        return

    members = workspace.get("members")
    if members is None:
        return

    retained = [
        entry
        for entry in members
        if isinstance(entry, str) and Path(entry).name in PUBLISHABLE_CRATES
    ]

    if list(members) == retained:
        return

    replacement = array()
    replacement.extend(retained)
    workspace["members"] = replacement
    manifest.write_text(dumps(document), encoding="utf-8")


def apply_workspace_replacements(
    workspace_root: Path,
    version: str,
    *,
    include_local_path: bool,
    crates: tuple[str, ...] | None = None,
) -> None:
    """Rewrite workspace dependency declarations for publish workflows.

    Parameters
    ----------
    workspace_root : Path
        Root of the exported workspace containing crate directories.
    version : str
        Version string written to dependency entries.
    include_local_path : bool
        When ``True`` the relative ``path`` entries are retained so dry-run
        checks use the local workspace.
    crates : tuple[str, ...] | None, optional
        Specific crates to update. Defaults to all known crates when ``None``.

    Returns
    -------
    None
        Each targeted manifest is rewritten in place.
    """
    targets = REPLACEMENTS if crates is None else crates
    for crate in targets:
        if crate not in REPLACEMENTS:
            continue
        manifest = workspace_root / "crates" / crate / "Cargo.toml"
        apply_replacements(
            crate,
            manifest,
            version,
            include_local_path=include_local_path,
        )


def workspace_version(manifest: Path) -> str:
    """Return the workspace package version from the root manifest.

    Parameters
    ----------
    manifest : Path
        Path to the workspace ``Cargo.toml`` file.

    Returns
    -------
    str
        The semantic version configured under ``[workspace.package]``.

    Raises
    ------
    SystemExit
        Raised when the workspace manifest lacks the version entry.
    """
    data = tomllib.loads(manifest.read_text(encoding="utf-8"))
    try:
        return data["workspace"]["package"]["version"]
    except KeyError as err:
        message = (
            f"expected [workspace.package].version in {manifest}; "
            "[workspace.package] must define a version for publish automation to run. "
            "Check the manifest defines the key."
        )
        raise SystemExit(message) from err


def remove_patch_entry(manifest: Path, crate: str) -> None:
    """Remove the ``crate`` entry from the root ``[patch.crates-io]`` table.

    Parameters
    ----------
    manifest : Path
        Root workspace manifest that may contain patch overrides.
    crate : str
        Name of the crate whose patch entry should be removed.

    Returns
    -------
    None
        The manifest is rewritten only when the patch entry was present.
    """
    document = parse(manifest.read_text(encoding="utf-8"))
    patch_table = document.get("patch")
    if patch_table is None:
        return
    crates_io = patch_table.get("crates-io")
    if crates_io is None or crate not in crates_io:
        return
    del crates_io[crate]
    if not crates_io:
        del patch_table["crates-io"]
    if not patch_table:
        del document["patch"]
    manifest.write_text(dumps(document), encoding="utf-8")
