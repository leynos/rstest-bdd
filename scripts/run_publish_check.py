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
import tempfile
from contextlib import ExitStack
from dataclasses import dataclass
from pathlib import Path
from typing import Annotated, Callable, Optional, Protocol

import cyclopts
from cyclopts import App, Parameter
from plumbum import local
from plumbum.commands.processes import ProcessTimedOut

from publish_workspace import (
    apply_workspace_replacements,
    export_workspace,
    prune_workspace_members,
    remove_patch_entry,
    strip_patch_section,
    workspace_version,
)

Command = tuple[str, ...]


class CrateAction(Protocol):
    """Protocol describing callable crate actions used by workflow helpers."""

    def __call__(self, crate: str, workspace: Path, *, timeout_secs: int) -> None: ...

CRATE_ORDER: tuple[str, ...] = (
    "rstest-bdd-patterns",
    "rstest-bdd-macros",
    "rstest-bdd",
    "cargo-bdd",
)

LIVE_PUBLISH_COMMANDS: dict[str, tuple[Command, ...]] = {
    "rstest-bdd-patterns": (
        ("cargo", "publish", "--dry-run"),
        ("cargo", "publish"),
    ),
    "rstest-bdd-macros": (
        ("cargo", "publish", "--dry-run"),
        ("cargo", "publish"),
    ),
    "rstest-bdd": (
        ("cargo", "publish", "--dry-run"),
        ("cargo", "publish"),
    ),
    "cargo-bdd": (
        ("cargo", "publish", "--dry-run", "--locked"),
        ("cargo", "publish", "--locked"),
    ),
}

DEFAULT_PUBLISH_TIMEOUT_SECS = 900

app = App(config=cyclopts.config.Env("PUBLISH_CHECK_", command=False))


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
    """Log diagnostics for a failed Cargo command and abort execution.

    Parameters
    ----------
    crate
        Name of the crate whose Cargo invocation failed.
    result
        The :class:`CommandResult` describing the invocation, including the
        resolved command line and captured output streams.
    """

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


def publish_crate_commands(
    crate: str,
    workspace_root: Path,
    *,
    timeout_secs: int,
) -> None:
    """Run the configured live publish commands for ``crate``.

    The helper aborts with :class:`SystemExit` when the crate lacks a
    configured command sequence to ensure the workflow cannot silently skip
    releases when new crates are added to the workspace.
    """

    try:
        commands = LIVE_PUBLISH_COMMANDS[crate]
    except KeyError as error:
        raise SystemExit(f"missing live publish commands for {crate!r}") from error

    for command in commands:
        run_cargo_command(
            crate,
            workspace_root,
            list(command),
            timeout_secs=timeout_secs,
        )


@dataclass
class CrateProcessingConfig:
    """Configuration for crate processing workflow.

    Parameters
    ----------
    strip_patch : bool
        When ``True`` the ``[patch]`` section is removed before processing.
    include_local_path : bool
        Propagated to :func:`apply_workspace_replacements` to control whether
        crates retain local ``path`` overrides.
    apply_per_crate : bool
        When ``True`` workspace replacements are applied individually for each
        crate rather than once for the entire workspace.
    per_crate_cleanup : Callable[[Path, str], None] | None, optional
        Cleanup action executed after each crate has been processed.
    """

    strip_patch: bool
    include_local_path: bool
    apply_per_crate: bool
    per_crate_cleanup: Optional[Callable[[Path, str], None]] = None


def _process_crates(
    workspace: Path,
    timeout_secs: int,
    config: CrateProcessingConfig,
    crate_action: CrateAction,
) -> None:
    """Coordinate shared crate-processing workflow steps.

    Parameters
    ----------
    workspace : Path
        Path to the exported temporary workspace containing the Cargo manifest
        and crate directories.
    timeout_secs : int
        Timeout applied to each Cargo invocation triggered by the workflow.
    config : CrateProcessingConfig
        Declarative configuration describing how the workspace should be
        prepared and cleaned between crate actions.
    crate_action : CrateAction
        Callable invoked for each crate in :data:`CRATE_ORDER`.

    Examples
    --------
    Run a faux workflow that records the crates it sees::

        >>> tmp = Path("/tmp/workspace")  # doctest: +SKIP
        >>> config = CrateProcessingConfig(  # doctest: +SKIP
        ...     strip_patch=True,
        ...     include_local_path=True,
        ...     apply_per_crate=False
        ... )
        >>> _process_crates(  # doctest: +SKIP
        ...     tmp,
        ...     30,
        ...     config,
        ...     lambda crate, *_: None,
        ... )
    """

    manifest = workspace / "Cargo.toml"
    if config.strip_patch:
        strip_patch_section(manifest)
    version = workspace_version(manifest)

    def _apply_replacements(crate: Optional[str]) -> None:
        apply_workspace_replacements(
            workspace,
            version,
            include_local_path=config.include_local_path,
            crates=(crate,) if crate is not None else None,
        )

    if config.apply_per_crate:
        for crate in CRATE_ORDER:
            _apply_replacements(crate)
            crate_action(crate, workspace, timeout_secs=timeout_secs)
            if config.per_crate_cleanup is not None:
                config.per_crate_cleanup(manifest, crate)
    else:
        _apply_replacements(None)
        for crate in CRATE_ORDER:
            crate_action(crate, workspace, timeout_secs=timeout_secs)
            if config.per_crate_cleanup is not None:
                config.per_crate_cleanup(manifest, crate)


def _process_crates_for_live_publish(workspace: Path, timeout_secs: int) -> None:
    """Execute the live publish workflow for crates in release order.

    Parameters
    ----------
    workspace : Path
        Path to the exported temporary workspace containing the Cargo
        manifest and crate directories.
    timeout_secs : int
        Timeout applied to each Cargo invocation triggered by the workflow.

    Examples
    --------
    Trigger the live publish workflow after exporting the workspace::

        >>> tmp = Path("/tmp/workspace")  # doctest: +SKIP
        >>> _process_crates_for_live_publish(tmp, 900)  # doctest: +SKIP
    """

    config = CrateProcessingConfig(
        strip_patch=False,
        include_local_path=False,
        apply_per_crate=True,
        per_crate_cleanup=remove_patch_entry,
    )
    _process_crates(workspace, timeout_secs, config, publish_crate_commands)


def _process_crates_for_check(workspace: Path, timeout_secs: int) -> None:
    """Package or check crates locally to validate publish readiness.

    Parameters
    ----------
    workspace : Path
        Path to the exported temporary workspace containing the Cargo
        manifest and crate directories.
    timeout_secs : int
        Timeout applied to each Cargo invocation triggered by the workflow.

    Examples
    --------
    Package and check crates without publishing them::

        >>> tmp = Path("/tmp/workspace")  # doctest: +SKIP
        >>> _process_crates_for_check(tmp, 900)  # doctest: +SKIP
    """

    def _crate_action(crate: str, root: Path, *, timeout_secs: int) -> None:
        if crate == "rstest-bdd-patterns":
            package_crate(crate, root, timeout_secs=timeout_secs)
        else:
            check_crate(crate, root, timeout_secs=timeout_secs)

    config = CrateProcessingConfig(
        strip_patch=True,
        include_local_path=True,
        apply_per_crate=False,
    )
    _process_crates(workspace, timeout_secs, config, _crate_action)


def run_publish_check(*, keep_tmp: bool, timeout_secs: int, live: bool = False) -> None:
    """Run the publish workflow inside a temporary workspace directory.

    The default dry-run mode packages crates locally to validate publish
    readiness. Enable ``live`` to execute ``cargo publish`` for each crate in
    release order once the manifests have been rewritten for crates.io.

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
        if live:
            _process_crates_for_live_publish(workspace, timeout_secs)
        else:
            _process_crates_for_check(workspace, timeout_secs)
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
    live: Annotated[
        bool,
        Parameter(env_var="PUBLISH_CHECK_LIVE"),
    ] = False,
) -> None:
    """Cyclopts entry point for running the publish check workflow."""

    run_publish_check(keep_tmp=keep_tmp, timeout_secs=timeout_secs, live=live)


if __name__ == "__main__":
    app()
