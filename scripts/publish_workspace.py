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
from tomlkit.items import Array

if typ.TYPE_CHECKING:
    from tomlkit.toml_document import TOMLDocument

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
    document = parse(manifest.read_text(encoding="utf-8"))
    patch_table = document.get("patch")
    if patch_table is None:
        return

    crates_io = patch_table.get("crates-io")
    if crates_io is None:
        return

    del patch_table["crates-io"]
    if not patch_table:
        del document["patch"]

    rendered = dumps(document)
    if not rendered.endswith("\n"):
        rendered = f"{rendered}\n"

    manifest.write_text(rendered, encoding="utf-8")


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
    members = _get_valid_workspace_members(document)
    if members is None:
        return

    changed = _filter_workspace_members(members)
    _write_manifest_if_changed(
        document=document,
        manifest=manifest,
        changed=changed,
        members=members,
    )


def _get_valid_workspace_members(document: TOMLDocument) -> Array | None:
    """Return the workspace members array when it exists and is valid."""
    workspace = document.get("workspace")
    if workspace is None:
        return None

    members = workspace.get("members")
    if members is None:
        return None

    if isinstance(members, Array):
        return members

    if isinstance(members, list):
        rebuilt_members = array()
        rebuilt_members.extend(members)
        workspace["members"] = rebuilt_members
        return typ.cast("Array", rebuilt_members)

    return None


def _filter_workspace_members(members: Array) -> bool:
    """Remove ineligible workspace members, returning ``True`` if mutated."""
    changed = False
    for index in range(len(members) - 1, -1, -1):
        entry = members[index]
        if not isinstance(entry, str) or Path(entry).name not in PUBLISHABLE_CRATES:
            del members[index]
            changed = True

    return changed


def _write_manifest_if_changed(
    *, document: TOMLDocument, manifest: Path, changed: bool, members: Array
) -> None:
    """Persist ``document`` to ``manifest`` only when ``changed`` is ``True``."""
    if not changed:
        return

    workspace = document.get("workspace")
    if workspace is None:
        return

    rebuilt_members = array()
    rebuilt_members.extend(list(members))
    members_text = members.as_string()
    if "\n" in members_text:
        rebuilt_members.multiline(multiline=True)
    workspace["members"] = rebuilt_members

    rendered = dumps(document)
    if not rendered.endswith("\n"):
        rendered = f"{rendered}\n"

    manifest.write_text(rendered, encoding="utf-8")


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
    manifest_text = manifest.read_text(encoding="utf-8")
    data = tomllib.loads(manifest_text)
    try:
        return data["workspace"]["package"]["version"]
    except KeyError as err:
        message = (
            f"expected [workspace.package].version in {manifest}; "
            "[workspace.package] must define a version for publish automation to run. "
            "Check the manifest defines the key."
        )
        snippet = _workspace_section_excerpt(manifest_text)
        if snippet:
            indented_snippet = "\n".join(f"    {line}" for line in snippet)
            message = f"{message}\n\nWorkspace manifest excerpt:\n{indented_snippet}"
        raise SystemExit(message) from err


def _workspace_section_excerpt(manifest_text: str) -> list[str] | None:
    """Return the lines around the ``[workspace]`` section for diagnostics."""
    lines = manifest_text.splitlines()
    workspace_index = _find_workspace_section_index(lines)

    if workspace_index is None:
        return None

    return _extract_section_lines(lines, workspace_index)


def _find_workspace_section_index(lines: list[str]) -> int | None:
    """Find the index of the [workspace] section."""
    for index, line in enumerate(lines):
        if line.strip().startswith("[workspace"):
            return index
    return None


def _extract_section_lines(lines: list[str], workspace_index: int) -> list[str]:
    """Extract lines around the workspace section for diagnostics."""
    start = max(workspace_index - 1, 0)
    end = workspace_index + 1

    while end < len(lines) and end - start < 8:
        stripped = lines[end].strip()
        if stripped.startswith("[") and not stripped.startswith("[workspace"):
            break
        end += 1

    return lines[start:end]


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
