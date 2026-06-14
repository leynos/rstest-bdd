"""Shared publish-check configuration and command context helpers.

This module centralizes the stable types, constants, and context-building
utilities used by the Cargo publish-check automation pipeline. It defines the
crate processing order, live publish command sets, already-published detection
markers, timeout resolution logic, and the command context passed to the
runtime executor.

Typical usage
-------------
Create a context before invoking a Cargo command:

>>> from pathlib import Path
>>> context = build_cargo_command_context("rstest-bdd", Path("/tmp/workspace"))
>>> context.crate
'rstest-bdd'
>>> context.crate_dir
PosixPath('/tmp/workspace/crates/rstest-bdd')
"""

from __future__ import annotations

import collections.abc as cabc
import dataclasses as dc
import logging
import os
import typing as typ

from publish_workspace import PUBLISHABLE_CRATES

if typ.TYPE_CHECKING:
    from pathlib import Path

LOGGER = logging.getLogger(__name__)

type Command = cabc.Sequence[str]
type CommandOutput = str | bytes


class CrateAction(typ.Protocol):
    """Protocol describing callable crate actions used by workflow helpers."""

    def __call__(self, crate: str, workspace_root: Path, *, timeout_secs: int) -> None:
        """Execute the action for ``crate`` within ``workspace_root``."""


CRATE_ORDER: typ.Final[tuple[str, ...]] = PUBLISHABLE_CRATES

DEFAULT_LIVE_PUBLISH_COMMANDS: typ.Final[tuple[Command, ...]] = (
    ("cargo", "publish", "--dry-run"),
    ("cargo", "publish"),
)

LIVE_PUBLISH_COMMANDS: typ.Final[dict[str, tuple[Command, ...]]] = dict.fromkeys(
    PUBLISHABLE_CRATES,
    DEFAULT_LIVE_PUBLISH_COMMANDS,
)

ALREADY_PUBLISHED_MARKERS: typ.Final[tuple[str, ...]] = (
    "already exists on crates.io index",
    "already exists on crates.io",
    "already uploaded",
    "already exists",
)
ALREADY_PUBLISHED_MARKERS_FOLDED: typ.Final[tuple[str, ...]] = tuple(
    marker.casefold() for marker in ALREADY_PUBLISHED_MARKERS
)

DEFAULT_PUBLISH_TIMEOUT_SECS: typ.Final[int] = 900


@dc.dataclass(frozen=True)
class CommandResult:
    """Result of a cargo command execution."""

    command: Command
    return_code: int
    stdout: CommandOutput
    stderr: CommandOutput


@dc.dataclass(frozen=True)
class CargoCommandContext:
    """Metadata describing where and how to run a Cargo command."""

    crate: str
    crate_dir: Path
    env_overrides: cabc.Mapping[str, str]
    timeout_secs: int


@dc.dataclass(frozen=True)
class CargoExecutionContext:
    """Context for executing cargo commands in a workspace."""

    crate: str
    workspace_root: Path
    timeout_secs: int | None = None


type FailureHandler = cabc.Callable[[str, CommandResult], bool]


def _resolve_timeout(timeout_secs: int | None) -> int:
    """Return the timeout for Cargo commands."""
    if timeout_secs is not None:
        return timeout_secs

    env_value = os.environ.get("PUBLISH_CHECK_TIMEOUT_SECS")
    if env_value is None:
        return DEFAULT_PUBLISH_TIMEOUT_SECS

    try:
        return int(env_value)
    except ValueError as err:
        message = f"PUBLISH_CHECK_TIMEOUT_SECS must be an integer, got {env_value!r}"
        LOGGER.exception(message)
        raise SystemExit(message) from err


def build_cargo_command_context(
    crate: str,
    workspace_root: Path,
    *,
    crate_dir: Path | None = None,
    timeout_secs: int | None = None,
) -> CargoCommandContext:
    """Create the execution context for a Cargo command.

    Parameters
    ----------
    crate : str
        Name of the crate whose Cargo command will run.
    workspace_root : Path
        Root directory of the exported workspace.
    crate_dir : Path | None, optional
        Explicit directory for the command. When omitted, the directory is
        resolved as ``workspace_root / "crates" / crate``.
    timeout_secs : int | None, optional
        Command timeout in seconds. When omitted, the helper reads
        ``PUBLISH_CHECK_TIMEOUT_SECS`` or falls back to
        :data:`DEFAULT_PUBLISH_TIMEOUT_SECS`.

    Returns
    -------
    CargoCommandContext
        Context containing the crate name, command directory, environment
        overrides, and resolved timeout.
    """
    resolved_crate_dir = crate_dir or (workspace_root / "crates" / crate)
    env_overrides = {"CARGO_HOME": str(workspace_root / ".cargo-home")}
    resolved_timeout = _resolve_timeout(timeout_secs)
    return CargoCommandContext(
        crate=crate,
        crate_dir=resolved_crate_dir,
        env_overrides=env_overrides,
        timeout_secs=resolved_timeout,
    )
