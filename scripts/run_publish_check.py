#!/usr/bin/env -S uv run python
"""Automated publish-check workflow for Rust workspace crates."""

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
import shutil
import tempfile
import typing as typ
from pathlib import Path

import cyclopts
from cyclopts import App, Parameter
from plumbum import local
from plumbum.commands.processes import ProcessTimedOut
from publish_check_context import (
    CRATE_ORDER,
    DEFAULT_PUBLISH_TIMEOUT_SECS,
    LIVE_PUBLISH_COMMANDS,
    CargoCommandContext,
    CargoExecutionContext,
    Command,
    CommandResult,
    CrateAction,
    FailureHandler,
    build_cargo_command_context,
)
from publish_check_gpui import (
    GPUI_HARNESS_CRATE,
    build_packaged_archive,
    extract_packaged_archive,
    packaged_archive_path,
    write_validator_workspace,
)
from publish_check_gpui import (
    GPUI_VALIDATOR_CRATE as _GPUI_VALIDATOR_CRATE,
)
from publish_check_impl import (
    CargoRuntime,
    CrateProcessingDeps,
    CrateProcessingRequest,
    GpuiValidationDeps,
    execute_cargo_command_with_timeout,
    process_crates_impl,
    validate_packaged_gpui_harness_impl,
)
from publish_check_impl import (
    contains_already_published_marker as _contains_already_published_marker,
)
from publish_check_impl import (
    handle_command_failure as _handle_command_failure,
)
from publish_check_impl import (
    handle_command_output as _handle_command_output,
)
from publish_workspace import (
    apply_workspace_replacements,
    export_workspace,
    prune_workspace_members,
    remove_patch_entry,
    strip_patch_section,
    workspace_version,
)

LOGGER = logging.getLogger(__name__)
GPUI_VALIDATOR_CRATE = _GPUI_VALIDATOR_CRATE

if typ.TYPE_CHECKING:
    import collections.abc as cabc

app = App(
    config=cyclopts.config.Env("PUBLISH_CHECK_", command=False),
    result_action="sys_exit",
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
    return execute_cargo_command_with_timeout(
        context,
        command,
        CargoRuntime(local=local, timed_out_error=ProcessTimedOut, logger=LOGGER),
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


def run_cargo_command(
    context: CargoCommandContext,
    command: cabc.Sequence[str],
    *,
    on_failure: FailureHandler | None = None,
) -> None:
    """Run a Cargo command within the provided execution context."""
    _validate_cargo_command(command)

    result = _execute_cargo_command_with_timeout(context, command)

    _handle_cargo_result(context.crate, result, on_failure)


def _run_cargo_subcommand(
    context: CargoExecutionContext,
    subcommand: str,
    args: cabc.Sequence[str],
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
    args: cabc.Sequence[str],
    docstring: str,
) -> CrateAction:
    command_args = tuple(args)

    def action(
        crate: str,
        workspace_root: Path,
        *,
        timeout_secs: int,
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


def validate_packaged_gpui_harness(
    crate: str,
    workspace_root: Path,
    *,
    timeout_secs: int | None = None,
) -> None:
    """Package the GPUI harness crate and test the packaged artifact."""
    validate_packaged_gpui_harness_impl(
        crate,
        workspace_root,
        timeout_secs=timeout_secs,
        deps=GpuiValidationDeps(
            workspace_version=workspace_version,
            packaged_archive_path=packaged_archive_path,
            build_packaged_archive=build_packaged_archive,
            extract_packaged_archive=extract_packaged_archive,
            write_validator_workspace=write_validator_workspace,
            build_cargo_command_context=build_cargo_command_context,
            run_cargo_command=run_cargo_command,
        ),
    )


def _publish_one_command(
    crate: str,
    workspace_root: Path,
    command: cabc.Sequence[str],
    timeout_secs: int | None = None,
) -> bool:
    """Run a publish command, returning ``True`` when publishing should stop."""
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
    """Run the configured live publish commands for ``crate``."""
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
    """Configuration for crate processing workflow."""

    strip_patch: bool
    include_local_path: bool
    apply_per_crate: bool
    per_crate_cleanup: cabc.Callable[[Path, str], None] | None = None


def _process_crates(
    workspace: Path,
    timeout_secs: int,
    config: CrateProcessingConfig,
    crate_action: CrateAction,
) -> None:
    """Coordinate shared crate-processing workflow steps."""
    process_crates_impl(
        CrateProcessingRequest(
            workspace=workspace,
            timeout_secs=timeout_secs,
            config=config,
            crate_action=crate_action,
        ),
        CrateProcessingDeps(
            crate_order=CRATE_ORDER,
            strip_patch_section=strip_patch_section,
            workspace_version=workspace_version,
            apply_workspace_replacements=apply_workspace_replacements,
        ),
    )


def _process_crates_for_live_publish(workspace: Path, timeout_secs: int) -> None:
    """Execute the live publish workflow for crates in release order."""
    config = CrateProcessingConfig(
        strip_patch=False,
        include_local_path=False,
        apply_per_crate=True,
        per_crate_cleanup=remove_patch_entry,
    )
    _process_crates(workspace, timeout_secs, config, publish_crate_commands)


def _process_crates_for_check(workspace: Path, timeout_secs: int) -> None:
    """Package or check crates locally to validate publish readiness."""

    def _resolve_check_action(crate: str) -> CrateAction:
        def _validate_gpui_harness(
            crate: str,
            workspace_root: Path,
            *,
            timeout_secs: int,
        ) -> None:
            validate_packaged_gpui_harness(
                crate,
                workspace_root,
                timeout_secs=timeout_secs,
            )

        special_actions: dict[str, CrateAction] = {
            "rstest-bdd-patterns": package_crate,
            GPUI_HARNESS_CRATE: _validate_gpui_harness,
        }
        return special_actions.get(crate, check_crate)

    def _crate_action(crate: str, workspace_root: Path, *, timeout_secs: int) -> None:
        _resolve_check_action(crate)(
            crate,
            workspace_root,
            timeout_secs=timeout_secs,
        )

    config = CrateProcessingConfig(
        strip_patch=True,
        include_local_path=True,
        apply_per_crate=False,
    )
    _process_crates(workspace, timeout_secs, config, _crate_action)


def run_publish_check(*, keep_tmp: bool, timeout_secs: int, live: bool = False) -> None:
    """Run the publish workflow inside a temporary workspace directory."""
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
) -> int:
    """Run the publish-check CLI entry point."""
    run_publish_check(keep_tmp=keep_tmp, timeout_secs=timeout_secs, live=live)
    return 0


if __name__ == "__main__":
    app()
