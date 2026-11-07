"""A recording Cargo stand-in for the fixture prefetch's process boundary.

:mod:`test_fixture_lockfile_prefetch` covers the ``--fetch`` mode twice: in
process, against a mocked Cargo, and as a process, through a stand-in
``cargo`` reached on ``PATH``. Only the second needs a stand-in, and it needs
enough of one - the script, the environment it reads, the invocation log it
writes, and the parsers that separate a run's human wording from its metrics
record - that keeping it beside the tests would push that module past the
400-line budget the lint gate enforces.

Every helper here answers a question about a run its caller has already
started, so the only failures they produce are the skips for a platform whose
shell cannot run the stand-in.
"""

import json
import os
import shutil
import stat
import subprocess  # ruff: ignore[suspicious-subprocess-import] - tests run the trusted gate script and a stand-in, never foreign input.
import sys
from pathlib import Path

import pytest
from fixture_lockfile_reporting import PREFETCH_METRICS_PREFIX

REPO_ROOT = Path(__file__).resolve().parents[2]
GATE_SCRIPT = REPO_ROOT / "scripts" / "check_fixture_lockfiles.py"

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
