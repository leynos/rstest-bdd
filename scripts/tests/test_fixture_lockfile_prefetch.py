"""Unit tests for the standalone fixture dependency prefetch.

The mutation lane runs this mode before cargo-mutants so its nested fixture
builds resolve from a warm registry cache while ``--offline``; these tests pin
that the mode fetches every discovered fixture and reports failures without
validating anything itself. A subprocess run against a recording Cargo stand-in
covers the boundary the in-process tests mock out: the argv Cargo really
receives, the process exit status, and the two output streams.
"""

import os
import stat
import subprocess  # ruff: ignore[suspicious-subprocess-import] - tests build stand-in CompletedProcess values and run the trusted gate script.
import sys
from pathlib import Path
from unittest import mock

import pytest
from check_fixture_lockfiles import (
    cargo_fetch_command,
    fetch_fixtures,
    main,
)
from fixture_lockfile_discovery import discover_fixture_manifests

REPO_ROOT = Path(__file__).resolve().parents[2]
GATE_SCRIPT = REPO_ROOT / "scripts" / "check_fixture_lockfiles.py"
MAKEFILE = REPO_ROOT / "Makefile"

#: A discovered standalone fixture. The prefetch tests stand in for Cargo's
#: output, so any real fixture manifest serves as the one under test.
FIXTURE_MANIFEST = (
    REPO_ROOT / "crates/rstest-bdd/tests/fixtures/rebuild_invalidation/Cargo.toml"
)

FETCH_FAILURE = "error: failed to download `proc-macro-error-attr3 v3.1.0`"


def test_prefetch_fetches_every_discovered_fixture() -> None:
    """Every discovered fixture is warmed with locked ``cargo fetch``."""
    manifests = discover_fixture_manifests(REPO_ROOT)
    successful = subprocess.CompletedProcess(
        args=cargo_fetch_command(manifests[0]), returncode=0, stdout="", stderr=""
    )
    with mock.patch(
        "check_fixture_lockfiles.run_cargo_command", return_value=successful
    ) as runner:
        exit_code = fetch_fixtures(REPO_ROOT, manifests)
    assert exit_code == 0, "a clean prefetch must pass"
    assert [call.args for call in runner.call_args_list] == [
        (cargo_fetch_command(manifest), manifest) for manifest in manifests
    ], "the prefetch must warm every discovered fixture with its locked fetch argv"


def test_prefetch_failure_report_names_the_fixture_and_cargo_output() -> None:
    """A failed fetch names the fixture, the command, and Cargo's output."""
    manifest = FIXTURE_MANIFEST
    failing = subprocess.CompletedProcess(
        args=cargo_fetch_command(manifest),
        returncode=101,
        stdout="",
        stderr=FETCH_FAILURE,
    )
    with (
        mock.patch(
            "check_fixture_lockfiles.fetch_fixture_dependencies", return_value=failing
        ),
        mock.patch("check_fixture_lockfiles.print_failures") as emit_failures,
    ):
        exit_code = fetch_fixtures(REPO_ROOT, [manifest])
    assert exit_code == 1, "a failed prefetch must fail the run"
    report = emit_failures.call_args.args[0][0]
    assert report.startswith("failed to prefetch fixture dependencies: "), (
        "the prefetch must keep its own failure wording"
    )
    assert FIXTURE_MANIFEST.relative_to(REPO_ROOT).as_posix() in report, (
        "the report must name the fixture whose dependencies did not download"
    )
    assert f"command: {' '.join(cargo_fetch_command(manifest))}" in report, (
        "the report must quote the command for reproduction"
    )
    assert FETCH_FAILURE in report, "the report must carry Cargo stderr"


def test_prefetch_failure_summary_streams_to_stderr(
    capsys: pytest.CaptureFixture[str],
) -> None:
    """A failed prefetch surfaces its summary on standard error, exit code 1."""
    manifest = FIXTURE_MANIFEST
    failing = subprocess.CompletedProcess(
        args=cargo_fetch_command(manifest),
        returncode=101,
        stdout="",
        stderr=FETCH_FAILURE,
    )
    with mock.patch(
        "check_fixture_lockfiles.fetch_fixture_dependencies", return_value=failing
    ):
        exit_code = fetch_fixtures(REPO_ROOT, [manifest])
    assert exit_code == 1, "a failed prefetch must fail the run"
    captured = capsys.readouterr()
    assert "1 of 1 fixture dependency prefetch(es) failed" in captured.err, (
        "the prefetch summary must report the failed count on standard error"
    )


def test_main_fetch_flag_routes_to_prefetch_without_validating(
    capsys: pytest.CaptureFixture[str],
) -> None:
    """--fetch warms the cache instead of validating the lockfiles."""
    manifests = discover_fixture_manifests(REPO_ROOT)
    successful = subprocess.CompletedProcess(
        args=cargo_fetch_command(manifests[0]), returncode=0, stdout="", stderr=""
    )
    with (
        mock.patch(
            "check_fixture_lockfiles.discover_fixture_manifests", return_value=manifests
        ),
        mock.patch(
            "check_fixture_lockfiles.fetch_fixture_dependencies",
            return_value=successful,
        ),
        mock.patch("check_fixture_lockfiles.run_cargo_metadata") as metadata,
    ):
        exit_code = main(["--fetch"])
    assert exit_code == 0, "a clean prefetch must exit zero"
    assert metadata.call_count == 0, (
        "the prefetch mode must not stand in for lockfile validation"
    )
    assert f"prefetched dependencies for {len(manifests)} fixture(s)" in (
        capsys.readouterr().out
    ), "the prefetch summary must confirm the run"


def test_cargo_fetch_command_pins_the_locked_fetch_argv() -> None:
    """The prefetch argv is the locked fetch for exactly one manifest."""
    assert cargo_fetch_command(FIXTURE_MANIFEST) == [
        "cargo",
        "fetch",
        "--locked",
        "--manifest-path",
        str(FIXTURE_MANIFEST),
    ], "the prefetch must pin the locked fetch argv the mutation lane relies on"


def makefile_recipe(target: str) -> str:
    """Return the tab-indented recipe of the named Makefile target."""
    lines = MAKEFILE.read_text(encoding="utf-8").splitlines()
    start = next(
        (index for index, line in enumerate(lines) if line.startswith(f"{target}:")),
        None,
    )
    assert start is not None, f"the Makefile must define the {target} target"
    recipe: list[str] = []
    for line in lines[start + 1 :]:
        if not line.startswith("\t"):
            break
        recipe.append(line.strip())
    assert recipe, f"the {target} target must have a recipe"
    return "\n".join(recipe)


def test_makefile_prefetch_target_runs_the_fetch_mode() -> None:
    """`make prefetch-fixture-deps` drives the gate's ``--fetch`` mode."""
    recipe = makefile_recipe("prefetch-fixture-deps")

    assert "scripts/check_fixture_lockfiles.py" in recipe, (
        "the mutation lane's prefetch must run the fixture-lockfile gate script"
    )
    assert "--fetch" in recipe, "the prefetch target must select the fetch mode"


#: A Cargo stand-in that records each argument vector it receives and fails
#: only for the manifest its caller names in FAKE_CARGO_FAIL_MATCH.
FAKE_CARGO = """\
#!/usr/bin/env sh
status=0
for argument in "$@"; do
    printf "%s\\0" "$argument" >> "$FAKE_CARGO_LOG"
    if [ "$argument" = "$FAKE_CARGO_FAIL_MATCH" ]; then
        printf "error: failed to download fixture dependency\\n" >&2
        status=101
    fi
done
printf "\\n" >> "$FAKE_CARGO_LOG"
exit "$status"
"""


def run_fetch_with_fake_cargo(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
    failing: Path | None = None,
) -> tuple[subprocess.CompletedProcess[str], list[list[str]]]:
    """Run the gate's ``--fetch`` mode against a recording Cargo stand-in.

    Returns
    -------
    tuple[subprocess.CompletedProcess[str], list[list[str]]]
        The gate's completed run and the argument vector of every Cargo
        invocation it made, in order.
    """
    if os.name == "nt":
        pytest.skip("the recording Cargo stand-in is a POSIX shell script")
    cargo_directory = tmp_path / "bin"
    cargo_directory.mkdir()
    fake_cargo = cargo_directory / "cargo"
    fake_cargo.write_text(FAKE_CARGO, encoding="utf-8")
    fake_cargo.chmod(fake_cargo.stat().st_mode | stat.S_IXUSR)
    invocation_log = tmp_path / "cargo-invocations.log"
    monkeypatch.setenv("PATH", f"{cargo_directory}{os.pathsep}{os.environ['PATH']}")
    monkeypatch.setenv("FAKE_CARGO_LOG", str(invocation_log))
    monkeypatch.setenv("FAKE_CARGO_FAIL_MATCH", "" if failing is None else str(failing))

    result = subprocess.run(  # ruff: ignore[subprocess-without-shell-equals-true] - the argv is this test's own command.
        [sys.executable, str(GATE_SCRIPT), "--fetch"],
        cwd=REPO_ROOT,
        env=os.environ.copy(),
        text=True,
        capture_output=True,
        check=False,
        timeout=120,
    )
    records = (
        invocation_log.read_text(encoding="utf-8").splitlines()
        if invocation_log.exists()
        else []
    )
    return result, [record.rstrip("\0").split("\0") for record in records if record]


def test_fetch_mode_warms_every_fixture_through_the_locked_fetch_argv(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    """The script fetches each discovered fixture once and exits zero."""
    manifests = discover_fixture_manifests(REPO_ROOT)

    result, invocations = run_fetch_with_fake_cargo(tmp_path, monkeypatch)

    assert result.returncode == 0, result.stderr
    assert invocations == [
        ["fetch", "--locked", "--manifest-path", str(manifest)]
        for manifest in manifests
    ], "the gate must run one locked fetch per discovered fixture, in order"
    summary = f"prefetched dependencies for {len(manifests)} fixture(s)"
    assert summary in result.stdout, (
        "a clean prefetch must confirm its count on standard output"
    )


def test_fetch_mode_reports_a_failing_fixture_and_exits_nonzero(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    """A fixture whose download fails is reported, and the run still completes."""
    manifests = discover_fixture_manifests(REPO_ROOT)
    failing = manifests[0]

    result, invocations = run_fetch_with_fake_cargo(
        tmp_path, monkeypatch, failing=failing
    )

    assert result.returncode == 1, "a failed prefetch must fail the gate"
    assert len(invocations) == len(manifests), (
        "one failing fixture must not stop the remaining prefetches"
    )
    relative = failing.relative_to(REPO_ROOT).as_posix()
    assert f"failed to prefetch fixture dependencies: {relative}" in result.stderr, (
        "the report must name the fixture whose dependencies did not download"
    )
    command = f"command: cargo fetch --locked --manifest-path {failing}"
    assert command in result.stderr, (
        "the report must quote the command the gate ran for reproduction"
    )
    assert "error: failed to download fixture dependency" in result.stderr, (
        "the report must carry Cargo's own diagnostics"
    )
    summary = f"1 of {len(manifests)} fixture dependency prefetch(es) failed"
    assert summary in result.stderr, (
        "the summary must count the failed prefetch on standard error"
    )
