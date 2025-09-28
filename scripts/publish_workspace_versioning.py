"""Workspace version diagnostics for publish automation.

These helpers provide access to the workspace package version and produce
contextual diagnostics when required keys are missing.
"""

from __future__ import annotations

from pathlib import Path

import tomllib

__all__ = [
    "_extract_section_lines",
    "_find_workspace_section_index",
    "_should_include_more_lines",
    "_workspace_section_excerpt",
    "workspace_version",
]


def workspace_version(manifest: Path) -> str:
    """Return the workspace package version from the root manifest."""
    manifest = Path(manifest)
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
        if snippet := _workspace_section_excerpt(manifest_text):
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

    while _should_include_more_lines(lines, end, start):
        end += 1

    return lines[start:end]


def _should_include_more_lines(lines: list[str], end: int, start: int) -> bool:
    """Return ``True`` when diagnostic extraction should continue."""
    if end >= len(lines):
        return False

    if end - start >= 8:
        return False

    stripped = lines[end].strip()
    is_non_workspace_section = stripped.startswith("[") and not stripped.startswith(
        "[workspace"
    )
    return not is_non_workspace_section
