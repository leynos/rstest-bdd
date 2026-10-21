"""Unit tests for the Cargo invocation layer of the fixture-lockfile gate.

Each operation the gate runs over a fixture is a fixed argv plus the manifest
path, routed through one shared runner so a lockfile that cannot be validated
is reported per fixture rather than aborting the run. These tests pin that
routing and the launch-failure contract; the gate's mode behaviour lives in
``test_check_fixture_lockfiles``.
"""

from pathlib import Path
from unittest import mock

import pytest
from fixture_lockfile_cargo import (
    cargo_fetch_command,
    cargo_metadata_command,
    cargo_resolve_command,
    fetch_fixture_dependencies,
    refresh_lockfile,
    run_cargo_command,
    run_cargo_metadata,
)
from fixture_lockfile_reporting import FixtureLockfileError

REPO_ROOT = Path(__file__).resolve().parents[2]


def test_run_cargo_command_reports_a_missing_cargo_executable() -> None:
    """A Cargo that cannot start raises the gate's own error, named per fixture."""
    manifest = REPO_ROOT / "crates/rstest-bdd/tests/ui_lints/Cargo.toml"
    oserror = OSError(2, "No such file or directory")
    with (
        mock.patch(
            "fixture_lockfile_cargo.subprocess.run", side_effect=oserror
        ) as spawn,
        pytest.raises(FixtureLockfileError, match="cannot run cargo") as excinfo,
    ):
        run_cargo_command(cargo_metadata_command(manifest), manifest)
    assert spawn.call_args.kwargs["capture_output"] is True, (
        "the runner must capture Cargo output so failures can name it"
    )
    assert spawn.call_args.kwargs["text"] is True, (
        "Cargo output must be decoded as text"
    )
    assert spawn.call_args.kwargs["check"] is False, (
        "a non-zero exit must return the result so every fixture is reported"
    )
    assert isinstance(excinfo.value.__cause__, OSError), (
        "the original launch failure must stay chained for debugging"
    )


def test_thin_wrappers_delegate_to_the_shared_runner() -> None:
    """Every public entry point builds its argv and hands it to one runner."""
    manifest = REPO_ROOT / "crates/rstest-bdd/tests/ui_lints/Cargo.toml"
    with mock.patch("fixture_lockfile_cargo.run_cargo_command") as runner:
        run_cargo_metadata(manifest)
        refresh_lockfile(manifest)
        fetch_fixture_dependencies(manifest)
    assert runner.call_count == 3, (
        "each wrapper must route through the shared Cargo runner exactly once"
    )
    metadata_argv, metadata_manifest = runner.call_args_list[0].args
    refresh_argv, refresh_manifest = runner.call_args_list[1].args
    fetch_argv, fetch_manifest = runner.call_args_list[2].args
    assert metadata_argv == cargo_metadata_command(manifest), (
        "validation must always use the locked metadata argv"
    )
    assert metadata_manifest == manifest, "validation must name the manifest"
    assert refresh_argv == cargo_resolve_command(manifest), (
        "the refresh must re-resolve through cargo metadata, not "
        "generate-lockfile: only the former keeps the versions the existing "
        "lockfile pins"
    )
    assert refresh_manifest == manifest, "refresh must name the manifest it regenerated"
    assert fetch_argv == cargo_fetch_command(manifest), (
        "the prefetch must use the locked fetch argv"
    )
    assert fetch_manifest == manifest, "the prefetch must name the manifest it warmed"
