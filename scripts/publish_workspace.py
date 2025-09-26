"""Workspace preparation helpers shared by publish automation scripts.

The routines in this module manipulate an exported copy of the repository so
release workflows can operate on a clean tree. They are intentionally
side-effect free apart from file-system writes inside the provided workspace
root, which keeps the publish scripts deterministic and safe to run locally.
"""

from __future__ import annotations

import tarfile
import tempfile
from pathlib import Path

import tomllib
from plumbum import local

from publish_patch import REPLACEMENTS, apply_replacements
from tomlkit import dumps, parse

PROJECT_ROOT = Path(__file__).resolve().parents[1]


def export_workspace(destination: Path) -> None:
    """Export the repository ``HEAD`` into ``destination``.

    Parameters
    ----------
    destination : Path
        Directory that receives the extracted workspace archive.

    Returns
    -------
    None
        The target directory is populated with the repository contents.

    Raises
    ------
    plumbum.commands.ProcessExecutionError
        Raised when ``git archive`` cannot be executed successfully.
    tarfile.TarError
        Propagated if the generated archive cannot be unpacked.
    """

    with tempfile.TemporaryDirectory() as archive_dir:
        archive_path = Path(archive_dir) / "workspace.tar"
        git_archive = local["git"][
            "archive",
            "--format=tar",
            "HEAD",
            f"--output={archive_path}",
        ]
        with local.cwd(PROJECT_ROOT):
            git_archive()
        with tarfile.open(archive_path) as tar:
            tar.extractall(destination, filter="data")


def _is_patch_section_start(line: str) -> bool:
    """Return True when the line marks the ``[patch.crates-io]`` section."""

    return line.strip() == "[patch.crates-io]"


def _is_any_section_start(line: str) -> bool:
    """Return True when the line starts a new manifest section."""

    return line.startswith("[")


def _process_patch_section_line(line: str, skipping_patch: bool) -> tuple[bool, bool]:
    """Process a line for patch section handling.

    Parameters
    ----------
    line
        The current line being processed.
    skipping_patch
        Current state indicating if we're inside a patch section.

    Returns
    -------
    tuple[bool, bool]
        A tuple of (should_include_line, new_skipping_patch_state).
    """

    if not skipping_patch and _is_patch_section_start(line):
        return False, True

    if skipping_patch and _is_any_section_start(line):
        return True, False

    return not skipping_patch, skipping_patch


def _ensure_proper_file_ending(lines: list[str]) -> None:
    """Ensure the file ends with a newline by adding an empty string if needed."""

    if not lines or lines[-1] != "":
        lines.append("")


def strip_patch_section(manifest: Path) -> None:
    """Remove the ``[patch.crates-io]`` section from ``manifest``.

    Parameters
    ----------
    manifest : Path
        Cargo manifest that will be rewritten in place.

    Returns
    -------
    None
        The manifest is updated so patch overrides are removed.
    """

    lines = manifest.read_text(encoding="utf-8").splitlines()
    cleaned: list[str] = []
    skipping_patch = False

    for line in lines:
        should_include, skipping_patch = _process_patch_section_line(line, skipping_patch)
        if should_include:
            cleaned.append(line)

    _ensure_proper_file_ending(cleaned)
    manifest.write_text("\n".join(cleaned), encoding="utf-8")


def _is_members_section_start(line: str) -> bool:
    """Return True if the line starts a workspace members section."""

    stripped = line.strip()
    return stripped.startswith("members") and stripped.endswith("[")


def _is_members_section_end(line: str) -> bool:
    """Return True if the line ends a workspace members section."""

    return line.strip() == "]"


def _should_include_member_line(line: str) -> bool:
    """Return True if the member entry references a crate directory."""

    return '"crates/' in line.strip()


def _process_member_line(line: str, inside_members: bool, result: list[str]) -> bool:
    """Update workspace member parsing state for a manifest line."""

    if _is_members_section_start(line):
        result.append(line)
        return True

    if inside_members and _is_members_section_end(line):
        result.append(line)
        return False

    if inside_members and not _should_include_member_line(line):
        return inside_members

    result.append(line)
    return inside_members


def prune_workspace_members(manifest: Path) -> None:
    """Remove non-crate entries from the workspace members list.

    Parameters
    ----------
    manifest : Path
        Workspace manifest whose ``members`` array should be filtered.

    Returns
    -------
    None
        The manifest is rewritten with only crate directories retained.
    """

    lines = manifest.read_text(encoding="utf-8").splitlines()
    result: list[str] = []
    inside_members = False
    for line in lines:
        inside_members = _process_member_line(line, inside_members, result)
    if result and result[-1] != "":
        result.append("")
    manifest.write_text("\n".join(result), encoding="utf-8")


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
        Root directory of the exported workspace that will be mutated.
    version : str
        Version string applied to rewritten dependencies.
    include_local_path : bool
        Forwarded to :func:`publish_patch.build_inline_dependency` to control
        whether rewritten dependencies retain their ``path`` entries.
    crates : tuple[str, ...], optional
        Subset of crates to update. Defaults to all entries in
        :data:`publish_patch.REPLACEMENTS` when omitted.

    Returns
    -------
    None
        Dependency manifests inside the workspace are patched in place.
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
    """Return the workspace package version from ``manifest``.

    Parameters
    ----------
    manifest : Path
        Root ``Cargo.toml`` path from which the version is read.

    Returns
    -------
    str
        Version string from ``[workspace.package].version``.

    Raises
    ------
    SystemExit
        Raised when the manifest is missing the ``workspace`` metadata.
    """

    data = tomllib.loads(manifest.read_text(encoding="utf-8"))
    try:
        return data["workspace"]["package"]["version"]
    except KeyError as err:
        raise SystemExit(f"expected [workspace.package].version in {manifest}") from err


def remove_patch_entry(manifest: Path, crate: str) -> None:
    """Remove ``crate`` from the root ``[patch.crates-io]`` table.

    Parameters
    ----------
    manifest : Path
        Workspace manifest whose patch section should be updated.
    crate : str
        Crate name whose override is removed after publishing.

    Returns
    -------
    None
        The manifest is rewritten with the updated patch table.
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
