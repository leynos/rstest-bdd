"""Unit tests for the standalone fixture dependency prefetch.

The mutation lane runs this mode before cargo-mutants so its nested fixture
builds resolve from a warm registry cache while ``--offline``; these tests pin
that the mode fetches every discovered fixture and reports failures without
validating anything itself. A subprocess run against a recording Cargo stand-in
covers the boundary the in-process tests mock out: the argv Cargo really
receives, the process exit status, and the two output streams.
"""

import json
import os
import shutil
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
from fixture_lockfile_reporting import PREFETCH_METRICS_PREFIX

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

    assert "scripts/check_fixture_lockfiles.py --fetch" in recipe, (
        "the mutation lane's prefetch must run the fixture-lockfile gate in "
        "its fetch mode"
    )


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
    *,
    via_make: bool = False,
) -> tuple[subprocess.CompletedProcess[str], list[list[str]]]:
    """Run the gate's ``--fetch`` mode against a recording Cargo stand-in.

    The stand-in shadows ``cargo`` on ``PATH``, so the platform's real Cargo
    never runs. ``via_make`` reaches the same mode the way the mutation lane
    does: through the ``prefetch-fixture-deps`` target.

    Returns
    -------
    tuple[subprocess.CompletedProcess[str], list[list[str]]]
        The gate's completed run and the argument vector of every Cargo
        invocation it made, in order.
    """
    if os.name == "nt":
        pytest.skip("the recording Cargo stand-in is a POSIX shell script")
    if via_make and shutil.which("make") is None:
        pytest.skip("the make-driven prefetch needs make on PATH")
    cargo_directory = tmp_path / "bin"
    cargo_directory.mkdir()
    fake_cargo = cargo_directory / "cargo"
    fake_cargo.write_text(FAKE_CARGO, encoding="utf-8")
    fake_cargo.chmod(fake_cargo.stat().st_mode | stat.S_IXUSR)
    invocation_log = tmp_path / "cargo-invocations.log"
    stand_in_path = f"{cargo_directory}{os.pathsep}{os.environ['PATH']}"
    monkeypatch.setenv("PATH", stand_in_path)
    monkeypatch.setenv("FAKE_CARGO_LOG", str(invocation_log))
    monkeypatch.setenv("FAKE_CARGO_FAIL_MATCH", "" if failing is None else str(failing))

    if via_make:
        # The Makefile prepends the real Cargo directory to PATH, so the
        # target's own PATH has to name the stand-in first. PROJECT_PYTHON
        # stands in for the uv launcher: the recipe the target runs is
        # otherwise untouched, and the test needs no toolchain of its own.
        command = [
            "make",
            "prefetch-fixture-deps",
            f"PATH={stand_in_path}",
            f"PROJECT_PYTHON={sys.executable}",
        ]
    else:
        command = [sys.executable, str(GATE_SCRIPT), "--fetch"]

    result = subprocess.run(  # ruff: ignore[subprocess-without-shell-equals-true] - the argv is this test's own command.
        command,
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


def metrics_records(text: str) -> list[dict[str, object]]:
    """Return every prefetch metrics record the *text* of one stream carries."""
    return [
        json.loads(line.removeprefix(PREFETCH_METRICS_PREFIX))
        for line in text.splitlines()
        if line.startswith(PREFETCH_METRICS_PREFIX)
    ]


def human_lines(text: str) -> list[str]:
    """Return the wording of *text*: its non-blank, non-metrics lines."""
    return [
        line
        for line in text.splitlines()
        if line and not line.startswith(PREFETCH_METRICS_PREFIX)
    ]


@pytest.mark.parametrize("failing_index", [None, 0], ids=["clean", "failing"])
def test_fetch_mode_emits_one_metrics_record_for_the_outcome(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch, failing_index: int | None
) -> None:
    """The run closes with one record of its counts on the outcome's stream."""
    manifests = discover_fixture_manifests(REPO_ROOT)
    failing = None if failing_index is None else manifests[failing_index]

    result, _ = run_fetch_with_fake_cargo(tmp_path, monkeypatch, failing=failing)

    failed = 0 if failing is None else 1
    assert result.returncode == failed, result.stderr
    outcome_stream = result.stdout if failing is None else result.stderr
    (record,) = metrics_records(outcome_stream)
    # Spelled out, not snapshotted: the field set is the contract under test.
    expected = {
        "schema_version": 1,
        "total": len(manifests),
        "succeeded": len(manifests) - failed,
        "failed": failed,
        "elapsed_ms": record["elapsed_ms"],
        "outcome": "failure" if failed else "success",
        "cache_outcome": "unknown",
    }
    assert record == expected, "the record must hold the run's counts and outcome"
    elapsed = record["elapsed_ms"]
    assert isinstance(elapsed, int), "the duration must be whole milliseconds"
    assert elapsed >= 0, "the record must carry a non-negative duration"
    other_stream = result.stderr if failing is None else result.stdout
    assert not metrics_records(other_stream), (
        "the record must ride the stream matching the outcome, never both"
    )


def test_metrics_record_carries_no_unbounded_values(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    """Paths, Cargo diagnostics, and command lines stay out of the record."""
    manifests = discover_fixture_manifests(REPO_ROOT)
    result, _ = run_fetch_with_fake_cargo(tmp_path, monkeypatch, failing=manifests[0])
    (line,) = [
        line
        for line in result.stderr.splitlines()
        if line.startswith(PREFETCH_METRICS_PREFIX)
    ]
    candidates = (
        str(REPO_ROOT),
        str(manifests[0]),
        FETCH_FAILURE,
        "error: failed to download fixture dependency",
    )
    leaked = [value for value in candidates if value in line]
    assert leaked == [], f"the record must carry counts only, never {leaked!r}"


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
    assert human_lines(result.stdout) == [
        f"prefetched dependencies for {len(manifests)} fixture(s)"
    ], "a clean prefetch keeps its exact success summary; the record joins it"


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
    # The established wording is the contract, so it is written out in full.
    expected = [
        f"failed to prefetch fixture dependencies: {relative}",
        f"command: cargo fetch --locked --manifest-path {failing}",
        "cargo output:",
        "error: failed to download fixture dependency",
        f"1 of {len(manifests)} fixture dependency prefetch(es) failed",
    ]
    assert human_lines(result.stderr) == expected, (
        "the metrics record must not reword the established failure report"
    )


def test_make_target_runs_the_fetch_mode_and_propagates_a_failure(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    """The mutation lane's target delegates to the gate and fails with it."""
    manifests = discover_fixture_manifests(REPO_ROOT)

    result, invocations = run_fetch_with_fake_cargo(
        tmp_path, monkeypatch, failing=manifests[0], via_make=True
    )

    assert result.returncode != 0, (
        "the prefetch target must fail the lane when a fixture does not download"
    )
    assert invocations == [
        ["fetch", "--locked", "--manifest-path", str(manifest)]
        for manifest in manifests
    ], "the target must run one locked fetch per discovered fixture, in order"
    assert "failed to prefetch fixture dependencies" in result.stderr, (
        "the target must let the gate's failure report reach the lane's log"
    )
    (record,) = metrics_records(result.stderr)
    assert record["total"] == len(manifests), "the record must count every fixture"
    assert record["failed"] == 1, "the record must count the failed fixture"
