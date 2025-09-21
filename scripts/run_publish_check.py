#!/usr/bin/env -S uv run python
# /// script
# requires-python = ">=3.11"
# dependencies = [
#     "tomlkit",
# ]
# ///
"""Run the publish-check workflow in a temporary workspace copy."""
from __future__ import annotations

import io
import os
import re
import shutil
import subprocess
import tarfile
import tempfile
import tomllib
from pathlib import Path

from publish_patch import REPLACEMENTS, apply_replacements

PUBLISH_CRATES = [
    "rstest-bdd-patterns",
    "rstest-bdd-macros",
    "rstest-bdd",
    "cargo-bdd",
]

PATCH_SECTION_PATTERN = re.compile(r"(?m)^\[patch\.crates-io\]\n(?:.*\n)*?(?=^\[|\Z)")
DEFAULT_PUBLISH_TIMEOUT_SECS = 900


def export_workspace(destination: Path) -> None:
    archive = subprocess.run(
        ["git", "archive", "--format=tar", "HEAD"],
        check=True,
        stdout=subprocess.PIPE,
    ).stdout
    with tarfile.open(fileobj=io.BytesIO(archive)) as tar:
        tar.extractall(destination, filter="data")


def strip_patch_section(manifest: Path) -> None:
    text = manifest.read_text(encoding="utf-8")
    cleaned, _ = PATCH_SECTION_PATTERN.subn("", text)
    if not cleaned.endswith("\n"):
        cleaned += "\n"
    manifest.write_text(cleaned, encoding="utf-8")


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


def prune_workspace_members(manifest: Path) -> None:
    lines = manifest.read_text(encoding="utf-8").splitlines()
    result: list[str] = []
    inside_members = False
    for line in lines:
        if _is_members_section_start(line):
            inside_members = True
            result.append(line)
            continue
        if inside_members and _is_members_section_end(line):
            inside_members = False
            result.append(line)
            continue
        if inside_members and not _should_include_member_line(line):
            continue
        result.append(line)
    if result and result[-1] != "":
        result.append("")
    manifest.write_text("\n".join(result), encoding="utf-8")


def apply_workspace_replacements(workspace_root: Path, version: str) -> None:
    for crate in REPLACEMENTS:
        manifest = workspace_root / "crates" / crate / "Cargo.toml"
        apply_replacements(crate, manifest, version)


def workspace_version(manifest: Path) -> str:
    data = tomllib.loads(manifest.read_text(encoding="utf-8"))
    try:
        return data["workspace"]["package"]["version"]
    except KeyError as err:
        raise SystemExit(f"expected [workspace.package].version in {manifest}") from err


def run_cargo_command(crate: str, workspace_root: Path, command: list[str]) -> None:
    """Run a Cargo command for a crate in the exported workspace.

    Parameters
    ----------
    crate
        Name of the crate located under the workspace's ``crates`` directory.
    workspace_root
        Root directory of the temporary workspace exported from the repository.
    command
        Command arguments, typically beginning with ``cargo``, to execute.

    The command honours ``PUBLISH_CHECK_TIMEOUT_SECS`` to avoid hanging CI runs.
    """

    crate_dir = workspace_root / "crates" / crate
    env = dict(os.environ)
    env["CARGO_HOME"] = str(workspace_root / ".cargo-home")
    timeout_value = os.environ.get("PUBLISH_CHECK_TIMEOUT_SECS")
    try:
        timeout = int(timeout_value) if timeout_value is not None else DEFAULT_PUBLISH_TIMEOUT_SECS
    except ValueError as err:
        raise SystemExit("PUBLISH_CHECK_TIMEOUT_SECS must be an integer") from err
    subprocess.run(command, check=True, cwd=crate_dir, env=env, timeout=timeout)


def package_crate(crate: str, workspace_root: Path) -> None:
    run_cargo_command(
        crate,
        workspace_root,
        ["cargo", "package", "--allow-dirty", "--no-verify"],
    )


def check_crate(crate: str, workspace_root: Path) -> None:
    run_cargo_command(
        crate,
        workspace_root,
        ["cargo", "check", "--all-features"],
    )


def main() -> None:
    workspace = Path(tempfile.mkdtemp())
    keep_workspace = bool(os.environ.get("PUBLISH_CHECK_KEEP_TMP"))
    try:
        export_workspace(workspace)
        manifest = workspace / "Cargo.toml"
        prune_workspace_members(manifest)
        strip_patch_section(manifest)
        version = workspace_version(manifest)
        apply_workspace_replacements(workspace, version)
        for crate in PUBLISH_CRATES:
            if crate == "rstest-bdd-patterns":
                package_crate(crate, workspace)
            else:
                check_crate(crate, workspace)
    finally:
        if keep_workspace:
            print(f"preserving workspace at {workspace}")
        else:
            shutil.rmtree(workspace, ignore_errors=True)


if __name__ == "__main__":
    main()
