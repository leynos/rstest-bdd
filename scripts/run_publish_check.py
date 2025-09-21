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
import logging
import os
import shlex
import shutil
import subprocess
import sys
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

DEFAULT_PUBLISH_TIMEOUT_SECS = 900


def export_workspace(destination: Path) -> None:
    archive = subprocess.run(
        ["git", "archive", "--format=tar", "HEAD"],
        check=True,
        stdout=subprocess.PIPE,
    ).stdout
    with tarfile.open(fileobj=io.BytesIO(archive)) as tar:
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
    """Strip the [patch.crates-io] section from a Cargo manifest file."""

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
    lines = manifest.read_text(encoding="utf-8").splitlines()
    result: list[str] = []
    inside_members = False
    for line in lines:
        inside_members = _process_member_line(line, inside_members, result)
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


def _parse_timeout_value() -> int:
    """Return the publish-check timeout derived from the environment."""

    timeout_value = os.environ.get("PUBLISH_CHECK_TIMEOUT_SECS")
    try:
        return (
            int(timeout_value)
            if timeout_value is not None
            else DEFAULT_PUBLISH_TIMEOUT_SECS
        )
    except ValueError as err:
        logging.error("PUBLISH_CHECK_TIMEOUT_SECS must be an integer: %s", err)
        raise SystemExit("PUBLISH_CHECK_TIMEOUT_SECS must be an integer") from err


def _handle_command_failure(
    crate: str, command: list[str], result: subprocess.CompletedProcess[str]
) -> None:
    """Log diagnostics for a failed Cargo command and raise its error."""

    logging.error(
        "cargo command failed for %s: %s",
        crate,
        shlex.join(command),
    )
    if result.stdout:
        logging.error("cargo stdout:%s%s", os.linesep, result.stdout)
    if result.stderr:
        logging.error("cargo stderr:%s%s", os.linesep, result.stderr)
    result.check_returncode()


def _handle_command_output(result: subprocess.CompletedProcess[str]) -> None:
    """Emit captured stdout and stderr from a successful Cargo command."""

    if result.stdout:
        print(result.stdout, end="")
    if result.stderr:
        print(result.stderr, end="", file=sys.stderr)


def run_cargo_command(crate: str, workspace_root: Path, command: list[str]) -> None:
    """Run a Cargo command for a crate in the exported workspace.

    Parameters
    ----------
    crate
        Name of the crate located under the workspace's ``crates`` directory.
    workspace_root
        Root directory of the temporary workspace exported from the repository.
    command
        Command arguments, which **must** begin with ``cargo``, to execute.

    Examples
    --------
    Running ``cargo --version`` for a crate directory:

    >>> run_cargo_command("tools", Path("/tmp/workspace"), ["cargo", "--version"])
    cargo 1.76.0 (9c9d2b9f8 2024-02-16)  # Version output will vary.

    The command honours ``PUBLISH_CHECK_TIMEOUT_SECS`` to avoid hanging CI runs.
    When the command fails, the captured stdout and stderr are logged to aid
    debugging in CI environments.
    """

    if not command or command[0] != "cargo":
        raise ValueError("run_cargo_command only accepts cargo invocations")

    crate_dir = workspace_root / "crates" / crate
    env = dict(os.environ)
    env["CARGO_HOME"] = str(workspace_root / ".cargo-home")
    timeout = _parse_timeout_value()

    result = subprocess.run(
        command,
        check=False,
        cwd=crate_dir,
        env=env,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
        timeout=timeout,
    )

    if result.returncode != 0:
        _handle_command_failure(crate, command, result)
        return

    _handle_command_output(result)


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
