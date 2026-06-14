"""Implementation helpers for the publish-check command entry point.

The public ``run_publish_check.py`` module keeps the command-line surface and
test monkeypatch seams stable.  This module owns the larger workflow helpers so
the facade can stay small while publish-check behaviour remains reusable from
tests and related packaging scripts.

Callers should normally use ``run_publish_check.main``.  Tests and local helper
code may import the focused functions here when they need to exercise a single
step, for example:

```
result = run_cargo_command(context, args, runtime)
```
"""

from __future__ import annotations

import dataclasses as dc
import logging
import os
import shlex
import sys
import typing as typ
from contextlib import ExitStack

from publish_check_context import (
    ALREADY_PUBLISHED_MARKERS_FOLDED,
    CargoCommandContext,
    Command,
    CommandOutput,
    CommandResult,
    CrateAction,
)
from publish_check_gpui import GPUI_HARNESS_CRATE, GPUI_VALIDATOR_CRATE

if typ.TYPE_CHECKING:
    import collections.abc as cabc
    from contextlib import AbstractContextManager
    from pathlib import Path

    class CargoInvocationLike(typ.Protocol):
        """Protocol for plumbum command invocations used by publish checks."""

        def run(
            self,
            *,
            retcode: None,
            timeout: int,
        ) -> tuple[int, str, str]:
            """Run the command and return process output."""

    class CargoCommandLike(typ.Protocol):
        """Protocol for plumbum command objects."""

        def __getitem__(self, args: Command) -> CargoInvocationLike:
            """Return an invocation with the supplied arguments."""

    class LocalLike(typ.Protocol):
        """Protocol for the subset of plumbum local used by publish checks."""

        def __getitem__(self, cmd: str) -> CargoCommandLike:
            """Return a command proxy."""

        def cwd(self, directory: Path) -> AbstractContextManager[object]:
            """Enter a working-directory context."""

        def env(self, **kwargs: str) -> AbstractContextManager[object]:
            """Enter an environment override context."""

    class CrateProcessingConfigLike(typ.Protocol):
        """Protocol for crate-processing configuration."""

        strip_patch: bool
        include_local_path: bool
        apply_per_crate: bool
        per_crate_cleanup: cabc.Callable[[Path, str], None] | None


LOGGER = logging.getLogger(__name__)


@dc.dataclass(frozen=True)
class CargoRuntime:
    """Runtime dependencies required to execute a Cargo command."""

    local: LocalLike
    timed_out_error: type[BaseException]
    logger: logging.Logger


@dc.dataclass(frozen=True)
class GpuiValidationDeps:
    """Collaborators required for GPUI packaged harness validation."""

    workspace_version: cabc.Callable[[Path], str]
    packaged_archive_path: cabc.Callable[[Path, str, str], Path]
    build_packaged_archive: cabc.Callable[..., Path]
    extract_packaged_archive: cabc.Callable[[Path, Path], Path]
    write_validator_workspace: cabc.Callable[..., Path]
    build_cargo_command_context: cabc.Callable[..., CargoCommandContext]
    run_cargo_command: cabc.Callable[..., None]


@dc.dataclass(frozen=True)
class CrateProcessingDeps:
    """Collaborators required by the shared crate-processing loop."""

    crate_order: tuple[str, ...]
    strip_patch_section: cabc.Callable[[Path], None]
    workspace_version: cabc.Callable[[Path], str]
    apply_workspace_replacements: cabc.Callable[..., None]


@dc.dataclass(frozen=True)
class CrateProcessingRequest:
    """Inputs for the shared crate-processing loop."""

    workspace: Path
    timeout_secs: int
    config: CrateProcessingConfigLike
    crate_action: CrateAction


def execute_cargo_command_with_timeout(
    context: CargoCommandContext,
    command: Command,
    runtime: CargoRuntime,
) -> CommandResult:
    """Run the Cargo command within the configured workspace context.

    Parameters
    ----------
    context : CargoCommandContext
        Resolved execution context for the crate and workspace.
    command : Command
        Cargo command sequence to execute.
    runtime : CargoRuntime
        Runtime collaborators used for command execution and diagnostics.

    Returns
    -------
    CommandResult
        Captured command line, return code, stdout, and stderr.

    Raises
    ------
    ValueError
        Raised when ``command`` is empty.
    SystemExit
        Raised when the Cargo command times out.
    """
    if not command:
        message = "cargo command must not be empty"
        raise ValueError(message)

    cargo_invocation = runtime.local[command[0]][command[1:]]
    try:
        with ExitStack() as stack:
            stack.enter_context(runtime.local.cwd(context.crate_dir))
            stack.enter_context(runtime.local.env(**context.env_overrides))
            return_code, stdout, stderr = cargo_invocation.run(
                retcode=None,
                timeout=context.timeout_secs,
            )
    except runtime.timed_out_error as error:
        runtime.logger.exception(
            "cargo command timed out for %s after %s seconds: %s",
            context.crate,
            context.timeout_secs,
            shlex.join(command),
        )
        message = (
            f"cargo command timed out for {context.crate!r} after "
            f"{context.timeout_secs} seconds"
        )
        raise SystemExit(message) from error

    return CommandResult(
        command=list(command),
        return_code=return_code,
        stdout=stdout,
        stderr=stderr,
    )


def validate_packaged_gpui_harness_impl(
    crate: str,
    workspace_root: Path,
    *,
    timeout_secs: int | None,
    deps: GpuiValidationDeps,
) -> None:
    """Package the GPUI harness crate and test the packaged artifact.

    Parameters
    ----------
    crate : str
        Crate name expected to match :data:`GPUI_HARNESS_CRATE`.
    workspace_root : Path
        Root directory of the exported publish-check workspace.
    timeout_secs : int | None
        Timeout forwarded to Cargo invocations.
    deps : GpuiValidationDeps
        Collaborators used to build, extract, and validate the package.

    Returns
    -------
    None
        The function performs validation through filesystem and Cargo side
        effects.

    Raises
    ------
    SystemExit
        Raised when ``crate`` is not the GPUI harness crate.
    """
    if crate != GPUI_HARNESS_CRATE:
        message = (
            "validate_packaged_gpui_harness expected "
            f"{GPUI_HARNESS_CRATE!r}, got {crate!r}"
        )
        raise SystemExit(message)

    manifest = workspace_root / "Cargo.toml"
    version = deps.workspace_version(manifest)
    archive = deps.packaged_archive_path(workspace_root, crate, version)
    deps.build_packaged_archive(
        workspace_root,
        archive,
        version,
        timeout_secs=timeout_secs,
    )

    validation_root = workspace_root / ".gpui-package-check"
    package_dir = deps.extract_packaged_archive(archive, validation_root / "package")
    validator_dir = deps.write_validator_workspace(
        validation_root / "validator",
        package_dir=package_dir,
        harness_dir=workspace_root / "crates" / "rstest-bdd-harness",
        version=version,
    )

    deps.run_cargo_command(
        deps.build_cargo_command_context(
            GPUI_VALIDATOR_CRATE,
            workspace_root,
            crate_dir=validator_dir,
            timeout_secs=timeout_secs,
        ),
        ["cargo", "check", "--tests"],
    )


def handle_command_failure(crate: str, result: CommandResult) -> None:
    """Log diagnostics for a failed Cargo command and abort execution.

    Parameters
    ----------
    crate : str
        Crate whose command failed.
    result : CommandResult
        Captured command result to report.

    Returns
    -------
    None
        This function always exits by raising :class:`SystemExit`.

    Raises
    ------
    SystemExit
        Raised with a message describing the failed command and exit code.
    """
    joined_command = shlex.join(result.command)
    LOGGER.error("cargo command failed for %s: %s", crate, joined_command)
    if result.stdout:
        LOGGER.error("cargo stdout:%s%s", os.linesep, result.stdout)
    if result.stderr:
        LOGGER.error("cargo stderr:%s%s", os.linesep, result.stderr)
    message = (
        f"cargo command failed for {crate!r}: {joined_command}"
        f" (exit code {result.return_code})"
    )
    raise SystemExit(message)


def _command_output_text(stream: CommandOutput) -> str:
    """Return command output as displayable text."""
    match stream:
        case bytes():
            return stream.decode("utf-8", errors="replace")
        case _:
            return stream


def handle_command_output(stdout: CommandOutput, stderr: CommandOutput) -> None:
    """Emit captured stdout and stderr from a successful Cargo command.

    Parameters
    ----------
    stdout : str | bytes
        Captured standard output.
    stderr : str | bytes
        Captured standard error.

    Returns
    -------
    None
        Output is written to the current process streams.
    """
    if stdout:
        print(_command_output_text(stdout), end="")
    if stderr:
        print(_command_output_text(stderr), end="", file=sys.stderr)


def contains_already_published_marker(result: CommandResult) -> bool:
    """Return whether Cargo output indicates the crate already exists.

    Parameters
    ----------
    result : CommandResult
        Captured publish command result.

    Returns
    -------
    bool
        ``True`` when stdout or stderr contains a known already-published
        marker from crates.io.
    """
    lowered_streams = (
        _command_output_text(stream).casefold()
        for stream in (result.stdout, result.stderr)
        if stream
    )
    return any(
        any(marker in lowered_stream for marker in ALREADY_PUBLISHED_MARKERS_FOLDED)
        for lowered_stream in lowered_streams
    )


def process_crates_impl(
    request: CrateProcessingRequest,
    deps: CrateProcessingDeps,
) -> None:
    """Coordinate shared crate-processing workflow steps.

    Parameters
    ----------
    request : CrateProcessingRequest
        Workspace, timeout, config, and crate action for the processing run.
    deps : CrateProcessingDeps
        Repository collaborators used to rewrite and inspect manifests.

    Returns
    -------
    None
        Crates are processed through the supplied action and manifest helpers.

    Raises
    ------
    SystemExit
        Raised when no crate order is configured.
    """
    if not deps.crate_order:
        message = "CRATE_ORDER must not be empty"
        raise SystemExit(message)

    manifest = request.workspace / "Cargo.toml"
    if request.config.strip_patch:
        deps.strip_patch_section(manifest)
    version = deps.workspace_version(manifest)

    if not request.config.apply_per_crate:
        deps.apply_workspace_replacements(
            request.workspace,
            version,
            include_local_path=request.config.include_local_path,
        )

    for crate in deps.crate_order:
        if request.config.apply_per_crate:
            deps.apply_workspace_replacements(
                request.workspace,
                version,
                include_local_path=request.config.include_local_path,
                crates=(crate,),
            )

        request.crate_action(
            crate,
            request.workspace,
            timeout_secs=request.timeout_secs,
        )

        if request.config.per_crate_cleanup is not None:
            request.config.per_crate_cleanup(manifest, crate)
