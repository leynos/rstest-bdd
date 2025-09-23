#!/usr/bin/env -S uv run python
"""Automated publish-check workflow for Rust workspace crates.

This module implements the publish-check automation that validates crate
packaging and compilation in an isolated workspace. The workflow exports
the repository to a temporary directory, strips patch sections, applies
version replacements, and validates each publishable crate.

The script supports timeout configuration via PUBLISH_CHECK_TIMEOUT_SECS
and workspace preservation via PUBLISH_CHECK_KEEP_TMP for debugging.

Examples
--------
Run the complete publish-check workflow::

    python scripts/run_publish_check.py

Run with custom timeout and workspace preservation::

    PUBLISH_CHECK_TIMEOUT_SECS=1200 PUBLISH_CHECK_KEEP_TMP=1 \
        python scripts/run_publish_check.py
"""

# /// script
# requires-python = ">=3.13"
# dependencies = [
#     "cyclopts>=2.9",
#     "plumbum",
#     "tomlkit",
# ]
# ///
from __future__ import annotations

import logging
import os
import shlex
import shutil
import sys
import tarfile
import tempfile
import tomllib
from contextlib import ExitStack
from dataclasses import dataclass
from pathlib import Path
from typing import Annotated

import cyclopts
from cyclopts import App, Parameter
from plumbum import local
from plumbum.commands.processes import ProcessTimedOut

from publish_patch import REPLACEMENTS, apply_replacements

PUBLISH_CRATES = [
    "rstest-bdd-patterns",
    "rstest-bdd-macros",
    "rstest-bdd",
    "cargo-bdd",
]

DEFAULT_PUBLISH_TIMEOUT_SECS = 900

PROJECT_ROOT = Path(__file__).resolve().parents[1]

app = App(config=cyclopts.config.Env("PUBLISH_CHECK_", command=False))


def export_workspace(destination: Path) -> None:
    """Extract the repository HEAD into ``destination`` via ``git archive``."""

    with tempfile.TemporaryDirectory() as archive_dir:
        archive_path = Path(archive_dir) / "workspace.tar"
        git_archive = local["git"]["archive", "--format=tar", "HEAD", f"--output={archive_path}"]
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


def _resolve_timeout(timeout_secs: int | None) -> int:
    """Return the timeout for Cargo commands.

    The value prioritises the explicit ``timeout_secs`` argument. When that is
    omitted, the ``PUBLISH_CHECK_TIMEOUT_SECS`` environment variable is
    consulted to preserve compatibility with the previous helper API before
    falling back to :data:`DEFAULT_PUBLISH_TIMEOUT_SECS`.
    """

    if timeout_secs is not None:
        return timeout_secs

    env_value = os.environ.get("PUBLISH_CHECK_TIMEOUT_SECS")
    if env_value is None:
        return DEFAULT_PUBLISH_TIMEOUT_SECS

    try:
        return int(env_value)
    except ValueError as err:
        logging.error("PUBLISH_CHECK_TIMEOUT_SECS must be an integer: %s", err)
        raise SystemExit("PUBLISH_CHECK_TIMEOUT_SECS must be an integer") from err


@dataclass(frozen=True)
class CommandResult:
    """Result of a cargo command execution."""

    command: list[str]
    return_code: int
    stdout: str
    stderr: str


def _handle_command_failure(
    crate: str,
    result: CommandResult,
) -> None:
    """Log diagnostics for a failed Cargo command and abort execution."""

    joined_command = shlex.join(result.command)
    logging.error("cargo command failed for %s: %s", crate, joined_command)
    if result.stdout:
        logging.error("cargo stdout:%s%s", os.linesep, result.stdout)
    if result.stderr:
        logging.error("cargo stderr:%s%s", os.linesep, result.stderr)
    raise SystemExit(
        f"cargo command failed for {crate!r}: {joined_command} (exit code {result.return_code})"
    )


def _handle_command_output(stdout: str, stderr: str) -> None:
    """Emit captured stdout and stderr from a successful Cargo command."""

    if stdout:
        print(stdout, end="")
    if stderr:
        print(stderr, end="", file=sys.stderr)


def run_cargo_command(
    crate: str,
    workspace_root: Path,
    command: list[str],
    *,
    timeout_secs: int | None = None,
) -> None:
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

    The command honours the ``timeout_secs`` parameter when provided. When it
    is omitted the ``PUBLISH_CHECK_TIMEOUT_SECS`` environment variable is
    consulted before falling back to the default. On failure the captured
    stdout and stderr are logged to aid debugging in CI environments.
    """

    if not command or command[0] != "cargo":
        raise ValueError("run_cargo_command only accepts cargo invocations")

    crate_dir = workspace_root / "crates" / crate
    env_overrides = {"CARGO_HOME": str(workspace_root / ".cargo-home")}

    cargo_invocation = local[command[0]][command[1:]]
    resolved_timeout = _resolve_timeout(timeout_secs)
    try:
        with ExitStack() as stack:
            stack.enter_context(local.cwd(crate_dir))
            stack.enter_context(local.env(**env_overrides))
            return_code, stdout, stderr = cargo_invocation.run(
                retcode=None,
                timeout=resolved_timeout,
            )
    except ProcessTimedOut as error:
        logging.error(
            "cargo command timed out for %s after %s seconds: %s",
            crate,
            resolved_timeout,
            shlex.join(command),
        )
        raise SystemExit(
            f"cargo command timed out for {crate!r} after {resolved_timeout} seconds"
        ) from error

    result = CommandResult(
        command=command,
        return_code=return_code,
        stdout=stdout,
        stderr=stderr,
    )
    if return_code != 0:
        _handle_command_failure(crate, result)
    else:
        _handle_command_output(result.stdout, result.stderr)


def package_crate(
    crate: str, workspace_root: Path, *, timeout_secs: int | None = None
) -> None:
    run_cargo_command(
        crate,
        workspace_root,
        ["cargo", "package", "--allow-dirty", "--no-verify"],
        timeout_secs=timeout_secs,
    )


def check_crate(
    crate: str, workspace_root: Path, *, timeout_secs: int | None = None
) -> None:
    run_cargo_command(
        crate,
        workspace_root,
        ["cargo", "check", "--all-features"],
        timeout_secs=timeout_secs,
    )


def run_publish_check(*, keep_tmp: bool, timeout_secs: int) -> None:
    """Run the publish workflow inside a temporary workspace directory.

    Examples
    --------
    Run the workflow and retain the temporary directory for manual inspection::

        >>> run_publish_check(keep_tmp=True, timeout_secs=120)
        preserving workspace at /tmp/...  # doctest: +ELLIPSIS
    """

    if timeout_secs <= 0:
        raise SystemExit("timeout-secs must be a positive integer")

    workspace = Path(tempfile.mkdtemp())
    try:
        export_workspace(workspace)
        manifest = workspace / "Cargo.toml"
        prune_workspace_members(manifest)
        strip_patch_section(manifest)
        version = workspace_version(manifest)
        apply_workspace_replacements(workspace, version)
        for crate in PUBLISH_CRATES:
            if crate == "rstest-bdd-patterns":
                package_crate(crate, workspace, timeout_secs=timeout_secs)
            else:
                check_crate(crate, workspace, timeout_secs=timeout_secs)
    finally:
        if keep_tmp:
            print(f"preserving workspace at {workspace}")
        else:
            shutil.rmtree(workspace, ignore_errors=True)


@app.default
def main(
    *,
    timeout_secs: Annotated[
        int,
        Parameter(env_var="PUBLISH_CHECK_TIMEOUT_SECS"),
    ] = DEFAULT_PUBLISH_TIMEOUT_SECS,
    keep_tmp: Annotated[
        bool,
        Parameter(env_var="PUBLISH_CHECK_KEEP_TMP"),
    ] = False,
) -> None:
    """Cyclopts entry point for running the publish check workflow."""

    run_publish_check(keep_tmp=keep_tmp, timeout_secs=timeout_secs)


if __name__ == "__main__":
    app()
