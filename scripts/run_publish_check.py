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

import dataclasses as dc
import logging
import os
import shlex
import shutil
import sys
import tempfile
import typing as typ
from contextlib import ExitStack
from pathlib import Path

import cyclopts
from cyclopts import App, Parameter
from plumbum import local
from plumbum.commands.processes import ProcessTimedOut
from publish_workspace import (
    PUBLISHABLE_CRATES,
    apply_workspace_replacements,
    export_workspace,
    prune_workspace_members,
    remove_patch_entry,
    strip_patch_section,
    workspace_version,
)

LOGGER = logging.getLogger(__name__)

Command = typ.Sequence[str]


class CrateAction(typ.Protocol):
    """Protocol describing callable crate actions used by workflow helpers."""

    def __call__(self, crate: str, workspace: Path, *, timeout_secs: int) -> None:
        """Execute the action for ``crate`` within ``workspace``."""
        ...


CRATE_ORDER: typ.Final[tuple[str, ...]] = PUBLISHABLE_CRATES

LOCKED_LIVE_CRATES: typ.Final[frozenset[str]] = frozenset({"cargo-bdd"})

DEFAULT_LIVE_CRATES: typ.Final[tuple[str, ...]] = tuple(
    crate for crate in PUBLISHABLE_CRATES if crate not in LOCKED_LIVE_CRATES
)

DEFAULT_LIVE_PUBLISH_COMMANDS: typ.Final[tuple[Command, ...]] = (
    ("cargo", "publish", "--dry-run"),
    ("cargo", "publish"),
)

LOCKED_LIVE_PUBLISH_COMMANDS: typ.Final[tuple[Command, ...]] = (
    ("cargo", "publish", "--dry-run", "--locked"),
    ("cargo", "publish", "--locked"),
)

LIVE_PUBLISH_COMMANDS: typ.Final[dict[str, tuple[Command, ...]]] = {
    crate: (
        LOCKED_LIVE_PUBLISH_COMMANDS
        if crate in LOCKED_LIVE_CRATES
        else DEFAULT_LIVE_PUBLISH_COMMANDS
    )
    for crate in PUBLISHABLE_CRATES
}

ALREADY_PUBLISHED_MARKERS: typ.Final[tuple[str, ...]] = (
    "already exists on crates.io index",
    "already exists on crates.io",
    "already uploaded",
    "already exists",
)
ALREADY_PUBLISHED_MARKERS_FOLDED: typ.Final[tuple[str, ...]] = tuple(
    marker.casefold() for marker in ALREADY_PUBLISHED_MARKERS
)

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
        LOGGER.exception("PUBLISH_CHECK_TIMEOUT_SECS must be an integer")
        message = "PUBLISH_CHECK_TIMEOUT_SECS must be an integer"
        raise SystemExit(message) from err


@dc.dataclass(frozen=True)
class CommandResult:
    """Result of a cargo command execution."""

    command: list[str]
    return_code: int
    stdout: str
    stderr: str


@dc.dataclass(frozen=True)
class CargoCommandContext:
    """Metadata describing where and how to run a Cargo command."""

    crate: str
    crate_dir: Path
    env_overrides: typ.Mapping[str, str]
    timeout_secs: int


FailureHandler = typ.Callable[[str, CommandResult], bool]


def build_cargo_command_context(
    crate: str,
    workspace_root: Path,
    *,
    timeout_secs: int | None = None,
) -> CargoCommandContext:
    """Create the execution context for a Cargo command.

    The helper resolves the workspace-relative crate directory, initialises the
    environment overrides, and normalises the timeout configuration to simplify
    subsequent :func:`run_cargo_command` invocations.

    Examples
    --------
    >>> context = build_cargo_command_context("tools", Path("/tmp/workspace"))
    >>> context.crate
    'tools'
    """
    crate_dir = workspace_root / "crates" / crate
    env_overrides = {"CARGO_HOME": str(workspace_root / ".cargo-home")}
    resolved_timeout = _resolve_timeout(timeout_secs)
    return CargoCommandContext(
        crate=crate,
        crate_dir=crate_dir,
        env_overrides=env_overrides,
        timeout_secs=resolved_timeout,
    )


def _validate_cargo_command(command: Command) -> None:
    """Ensure the provided command invokes Cargo."""
    if not command or command[0] != "cargo":
        message = "run_cargo_command only accepts cargo invocations"
        raise ValueError(message)


def _execute_cargo_command_with_timeout(
    context: CargoCommandContext,
    command: Command,
) -> CommandResult:
    """Run the Cargo command within the configured workspace context."""
    cargo_invocation = local[command[0]][command[1:]]
    try:
        with ExitStack() as stack:
            stack.enter_context(local.cwd(context.crate_dir))
            stack.enter_context(local.env(**context.env_overrides))
            return_code, stdout, stderr = cargo_invocation.run(
                retcode=None,
                timeout=context.timeout_secs,
            )
    except ProcessTimedOut as error:
        LOGGER.exception(
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


def _handle_cargo_result(
    crate: str,
    result: CommandResult,
    on_failure: FailureHandler | None,
) -> None:
    """Dispatch handling for successful and failed Cargo invocations."""
    if result.return_code == 0:
        _handle_command_output(result.stdout, result.stderr)
        return

    if on_failure is not None and on_failure(crate, result):
        return

    _handle_command_failure(crate, result)


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


def _handle_command_output(stdout: str, stderr: str) -> None:
    """Emit captured stdout and stderr from a successful Cargo command."""
    if stdout:
        print(stdout, end="")
    if stderr:
        print(stderr, end="", file=sys.stderr)


def run_cargo_command(
    context: CargoCommandContext,
    command: typ.Sequence[str],
    *,
    on_failure: FailureHandler | None = None,
) -> None:
    """Run a Cargo command within the provided execution context.

    Parameters
    ----------
    context
        Execution metadata returned by
        :func:`build_cargo_command_context`.
    command
        Command arguments, which **must** begin with ``cargo``, to execute.
    on_failure
        Optional callback that may handle command failures. When provided the
        handler receives the crate name and :class:`CommandResult`. Returning
        ``True`` suppresses the default error handling, allowing callers to
        decide whether execution should continue.

    Examples
    --------
    Running ``cargo --version`` for a crate directory:

    >>> context = build_cargo_command_context("tools", Path("/tmp/workspace"))
    >>> run_cargo_command(context, ["cargo", "--version"])
    cargo 1.76.0 (9c9d2b9f8 2024-02-16)  # Version output will vary.

    The command honours the ``timeout_secs`` parameter when provided. When it
    is omitted the ``PUBLISH_CHECK_TIMEOUT_SECS`` environment variable is
    consulted before falling back to the default. On failure the captured
    stdout and stderr are logged to aid debugging in CI environments.
    """
    _validate_cargo_command(command)

    result = _execute_cargo_command_with_timeout(context, command)

    _handle_cargo_result(context.crate, result, on_failure)


@dc.dataclass(frozen=True)
class CargoExecutionContext:
    """Context for executing cargo commands in a workspace."""

    crate: str
    workspace_root: Path
    timeout_secs: int | None = None


def _run_cargo_subcommand(
    context: CargoExecutionContext,
    subcommand: str,
    args: typ.Sequence[str],
) -> None:
    command = ["cargo", subcommand, *list(args)]
    run_cargo_command(
        build_cargo_command_context(
            context.crate,
            context.workspace_root,
            timeout_secs=context.timeout_secs,
        ),
        command,
    )


def _create_cargo_action(
    subcommand: str,
    args: typ.Sequence[str],
    docstring: str,
) -> CrateAction:
    command_args = tuple(args)

    def action(
        crate: str,
        workspace_root: Path,
        *,
        timeout_secs: int | None = None,
    ) -> None:
        context = CargoExecutionContext(
            crate,
            workspace_root,
            timeout_secs,
        )
        _run_cargo_subcommand(
            context,
            subcommand,
            command_args,
        )

    action.__doc__ = docstring
    return typ.cast("CrateAction", action)


package_crate = _create_cargo_action(
    "package",
    ["--allow-dirty", "--no-verify"],
    "Invoke ``cargo package`` for ``crate`` within the exported workspace.",
)


check_crate = _create_cargo_action(
    "check",
    ["--all-features"],
    "Run ``cargo check`` for ``crate`` using the exported workspace.",
)


def _contains_already_published_marker(result: CommandResult) -> bool:
    """Return ``True`` when Cargo output indicates the crate already exists."""
    for stream in (result.stdout, result.stderr):
        if not stream:
            continue

        if isinstance(stream, bytes):
            text = stream.decode("utf-8", errors="ignore")
        else:
            text = str(stream)

        lowered_stream = text.casefold()
        if any(marker in lowered_stream for marker in ALREADY_PUBLISHED_MARKERS_FOLDED):
            return True
    return False


def _publish_one_command(
    crate: str,
    workspace_root: Path,
    command: typ.Sequence[str],
    timeout_secs: int | None = None,
) -> bool:
    """Run a publish command, returning ``True`` when publishing should stop.

    When Cargo reports the crate version already exists on crates.io the
    captured output streams are replayed and a warning is emitted. The caller
    can then short-circuit the remaining publish commands for the crate.
    """
    handled = False

    def _on_failure(_crate: str, result: CommandResult) -> bool:
        nonlocal handled

        if not _contains_already_published_marker(result):
            return False

        handled = True
        _handle_command_output(result.stdout, result.stderr)
        LOGGER.warning(
            "crate %s already published on crates.io; skipping remaining commands",
            crate,
        )
        return True

    run_cargo_command(
        build_cargo_command_context(
            crate,
            workspace_root,
            timeout_secs=timeout_secs,
        ),
        command,
        on_failure=_on_failure,
    )
    return handled


def publish_crate_commands(
    crate: str,
    workspace_root: Path,
    *,
    timeout_secs: int,
) -> None:
    """Run the configured live publish commands for ``crate``.

    Parameters
    ----------
    crate : str
        Name of the crate being published. Must exist in
        :data:`LIVE_PUBLISH_COMMANDS`.
    workspace_root : Path
        Root directory containing the exported workspace.
    timeout_secs : int
        Timeout in seconds applied to each ``cargo publish`` invocation.

    Raises
    ------
    SystemExit
        Raised when ``crate`` has no live command sequence configured. The
        workflow aborts to avoid silently skipping new crates.
    """
    try:
        commands = LIVE_PUBLISH_COMMANDS[crate]
    except KeyError as error:
        message = f"missing live publish commands for {crate!r}"
        raise SystemExit(message) from error

    for command in commands:
        if _publish_one_command(
            crate,
            workspace_root,
            command,
            timeout_secs=timeout_secs,
        ):
            break


@dc.dataclass
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
    per_crate_cleanup: typ.Callable[[Path, str], None] | None = None


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
    if not CRATE_ORDER:
        message = "CRATE_ORDER must not be empty"
        raise SystemExit(message)

    manifest = workspace / "Cargo.toml"
    if config.strip_patch:
        strip_patch_section(manifest)
    version = workspace_version(manifest)

    if not config.apply_per_crate:
        apply_workspace_replacements(
            workspace,
            version,
            include_local_path=config.include_local_path,
        )

    for crate in CRATE_ORDER:
        if config.apply_per_crate:
            apply_workspace_replacements(
                workspace,
                version,
                include_local_path=config.include_local_path,
                crates=(crate,),
            )

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
        message = "timeout-secs must be a positive integer"
        raise SystemExit(message)

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
    timeout_secs: typ.Annotated[
        int,
        Parameter(env_var="PUBLISH_CHECK_TIMEOUT_SECS"),
    ] = DEFAULT_PUBLISH_TIMEOUT_SECS,
    keep_tmp: typ.Annotated[
        bool,
        Parameter(env_var="PUBLISH_CHECK_KEEP_TMP"),
    ] = False,
    live: typ.Annotated[
        bool,
        Parameter(env_var="PUBLISH_CHECK_LIVE"),
    ] = False,
) -> None:
    """Run the publish-check CLI entry point.

    Parameters
    ----------
    timeout_secs : int, optional
        Timeout in seconds for Cargo commands. Defaults to 900 seconds
        (``DEFAULT_PUBLISH_TIMEOUT_SECS``) and may be overridden via the
        ``PUBLISH_CHECK_TIMEOUT_SECS`` environment variable.
    keep_tmp : bool, optional
        When ``True`` the exported workspace directory is retained after the
        workflow finishes. Defaults to ``False`` and may also be set with the
        ``PUBLISH_CHECK_KEEP_TMP`` environment variable.
    live : bool, optional
        When ``True`` runs the live publish workflow instead of a dry run.
        Defaults to ``False`` and may be controlled through the
        ``PUBLISH_CHECK_LIVE`` environment variable.

    Returns
    -------
    None
        This function executes for its side effects and returns ``None``.
    """
    run_publish_check(keep_tmp=keep_tmp, timeout_secs=timeout_secs, live=live)


if __name__ == "__main__":
    app()
