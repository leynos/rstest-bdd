"""Unit tests for the prepared-fixture workflow's judgements.

The judgements in ``prepared_fixture_judgements`` are what decide whether the
workflow holds, and in the driver they run only against a real, cold target
directory — so a check that always passes would be invisible: the gate it
guards would simply look satisfied. The defect this file exists for was exactly
that shape. Step 3 scanned the captured output for ``Compiling`` lines, and
neither carrier it read could hold a nested compile: the helper that spawns
``cargo bdd steps`` discards its capture on success, and nextest suppresses a
passing test's output. The scan therefore found nothing whatever the build did
and read that emptiness as reuse.

Each case below drives one predicate against output that *is* the state under
test, in both directions, so a predicate that stopped discriminating fails
here rather than only in a cold end-to-end run nobody has recently afforded.
"""

import subprocess  # ruff: ignore[suspicious-subprocess-import] - only constructs CompletedProcess values; spawns nothing.
import sys
from pathlib import Path

import pytest

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from prepared_fixture_judgements import (
    StepFailureError,
    expect_pass_without_rebuild,
    expect_refusal,
    expect_stale_stamp_refusal,
)
from prepared_fixture_step import (
    BUILD_MARKER,
    REMEDY,
    TIMED_SOURCE,
    build_report_path,
    read_build_reports,
)


def a_result(
    returncode: int = 0,
    stdout: str = "",
    stderr: str = "",
) -> subprocess.CompletedProcess[str]:
    """Return a completed process shaped the way the driver receives one.

    Returns
    -------
    subprocess.CompletedProcess[str]
        A result carrying the supplied status and streams.
    """
    return subprocess.CompletedProcess(["step"], returncode, stdout, stderr)


def write_reports(target_dir: Path, *reports: tuple[str, int]) -> None:
    """Write one report line per ``(source, count)`` in *reports*, in order.

    The real producer appends, so a workflow's file holds one line per step,
    each naming the producer that filed it. Writing them in one call keeps each
    case's fixture legible while preserving that shape.
    """
    lines = [
        f"{BUILD_MARKER} source={source} compiling={count}\n"
        for source, count in reports
    ]
    build_report_path(target_dir).write_text("".join(lines), encoding="utf-8")


def test_a_refusal_naming_the_remedy_is_accepted() -> None:
    """The refusal the first step requires is accepted."""
    expect_refusal(a_result(returncode=100, stderr=f"run `{REMEDY}` first"))


def test_a_pass_is_not_read_as_a_refusal() -> None:
    """A pass against an empty target proves the gate is not refusing."""
    with pytest.raises(StepFailureError, match="passed against an empty target"):
        expect_refusal(a_result(returncode=0))


def test_a_drifted_filter_is_not_read_as_a_refusal() -> None:
    """``no tests to run`` exits non-zero without being a refusal.

    Treating the status alone as a refusal is the empty-filter hazard in its
    purest form: the gate would pass while exercising nothing.
    """
    result = a_result(returncode=4, stderr="error: no tests to run")
    with pytest.raises(StepFailureError, match="no tests to run"):
        expect_refusal(result)


def test_a_refusal_without_the_remedy_is_rejected() -> None:
    """A refusal must name the remedy, or the reader is left without one."""
    result = a_result(returncode=100, stderr="assertion failed: not prepared")
    with pytest.raises(StepFailureError, match="did not name"):
        expect_refusal(result)


def test_a_pass_reporting_zero_compiles_is_accepted(tmp_path: Path) -> None:
    """A pass whose report says nothing compiled is the expected reuse."""
    write_reports(tmp_path, ("timed", 0))
    expect_pass_without_rebuild(a_result(), target_dir=tmp_path, previous_counts=0)


def test_a_pass_reporting_a_compile_is_rejected(tmp_path: Path) -> None:
    """The negative control's outcome, in miniature.

    Without this case the predicate could satisfy itself by never failing.
    """
    write_reports(tmp_path, ("preparation", 0), ("timed", 7))
    with pytest.raises(StepFailureError, match="not reused"):
        expect_pass_without_rebuild(a_result(), target_dir=tmp_path, previous_counts=1)


def test_a_timed_command_that_compiled_is_rejected_despite_the_gates_zero(
    tmp_path: Path,
) -> None:
    """The case the gate's own count could never catch.

    A run files two lines: the gate's zero, because admitting a stamp means no
    preparation ran, then the timed command's own count. Reading only the first
    reported reuse while the command was free to rebuild, so a zero from the
    gate followed by a compile from the timed command must still fail.
    """
    write_reports(tmp_path, ("gate", 0), ("timed", 3))
    with pytest.raises(StepFailureError, match="not reused"):
        expect_pass_without_rebuild(a_result(), target_dir=tmp_path, previous_counts=0)


def test_both_counts_being_zero_is_the_accepted_reuse_shape(tmp_path: Path) -> None:
    """The positive control for the pair: gate zero and timed zero together."""
    write_reports(tmp_path, ("gate", 0), ("timed", 0))
    expect_pass_without_rebuild(a_result(), target_dir=tmp_path, previous_counts=0)


def test_a_gate_zero_with_no_timed_report_is_rejected(tmp_path: Path) -> None:
    """The gate's zero is not evidence about the timed command.

    This is the shape an earlier version of this check accepted: the gate files
    its zero before the test body runs, so a run that stopped, panicked or
    never reached ``cargo bdd steps`` leaves exactly that line and nothing
    else. Reading it as reuse makes the check about the gate rather than about
    the command it claims to judge.
    """
    write_reports(tmp_path, ("gate", 0))
    with pytest.raises(StepFailureError, match="no nested build from the timed"):
        expect_pass_without_rebuild(a_result(), target_dir=tmp_path, previous_counts=0)


def test_a_pass_with_no_report_is_rejected(tmp_path: Path) -> None:
    """Success with no evidence either way is a failure.

    This is the exact defect the file exists for: a missing report means the
    gate never ran, and reading that as "nothing compiled" is how a vacuous
    check passes.
    """
    with pytest.raises(StepFailureError, match="left no nested-build report"):
        expect_pass_without_rebuild(a_result(), target_dir=tmp_path, previous_counts=0)


def test_a_failed_run_is_rejected_before_the_report_is_read(tmp_path: Path) -> None:
    """A non-zero status is reported as such, not as a build problem."""
    write_reports(tmp_path, ("timed", 0))
    with pytest.raises(StepFailureError, match="does not admit the test"):
        expect_pass_without_rebuild(
            a_result(returncode=100), target_dir=tmp_path, previous_counts=0
        )


def test_a_report_line_this_reader_cannot_parse_is_a_failure(tmp_path: Path) -> None:
    """A producer-side rename must not degrade into silence again.

    The unmarked line stands for any format the reader no longer understands;
    it has to fail rather than leave the check vacuously satisfied.
    """
    build_report_path(tmp_path).write_text(
        f"{BUILD_MARKER} compiling=0\n", encoding="utf-8"
    )
    with pytest.raises(StepFailureError, match="left no nested-build report"):
        expect_pass_without_rebuild(a_result(), target_dir=tmp_path, previous_counts=0)


def test_an_earlier_steps_compile_does_not_fail_this_steps_reuse(
    tmp_path: Path,
) -> None:
    """The preparation's own cold build is not this step's rebuild.

    Step 2 compiles the graph and writes its count at the end of *that* run, so
    by the time step 3 is judged the file already holds a large nonzero line.
    Judging the whole file would fail step 3 for the preparation it depends on;
    only the lines added since the snapshot belong to the run under test.
    """
    write_reports(tmp_path, ("preparation", 51), ("timed", 0))
    expect_pass_without_rebuild(a_result(), target_dir=tmp_path, previous_counts=1)


def test_a_stale_stamp_refusal_naming_the_remedy_is_accepted() -> None:
    """The negative control's expected outcome is accepted."""
    expect_stale_stamp_refusal(
        a_result(returncode=100, stderr=f"stale stamp; run `{REMEDY}` first")
    )


def test_a_pass_after_the_edit_means_the_stamp_survived() -> None:
    """A pass proves the stamp ignored the input change.

    This is the direction that makes step 4 a control rather than a second
    positive assertion: if the stamp did not cover fixture sources, the edit
    would pass and the no-rebuild check would vouch for nothing.
    """
    with pytest.raises(StepFailureError, match="survived an input change"):
        expect_stale_stamp_refusal(a_result(returncode=0))


def test_a_failure_that_does_not_name_the_remedy_is_not_the_stamp() -> None:
    """A stale-stamp refusal must be distinguishable from any other failure.

    Exit status alone would accept a broken step, a missing tool, or a drifted
    filter — each of which leaves the control satisfied without testing the
    stamp.
    """
    with pytest.raises(StepFailureError, match="did not name"):
        expect_stale_stamp_refusal(a_result(returncode=100, stderr="segfault"))


def test_counts_are_returned_in_the_order_they_were_written(tmp_path: Path) -> None:
    """Order carries which step paid for a build, so it must be preserved."""
    write_reports(tmp_path, ("preparation", 12), ("gate", 0), ("timed", 3))
    reports = read_build_reports(tmp_path)
    assert [(r.source, r.compiling) for r in reports] == [
        ("preparation", 12),
        ("gate", 0),
        ("timed", 3),
    ], f"reports were reordered or lost: {reports}"


def test_an_absent_file_reads_as_no_reports(tmp_path: Path) -> None:
    """An unwritten report is no reports, not an empty success."""
    reports = read_build_reports(tmp_path)
    assert not reports, f"an absent report yielded reports: {reports}"


def test_unrelated_lines_are_ignored(tmp_path: Path) -> None:
    """A partially written or hand-inspected file must not confuse the reader."""
    build_report_path(tmp_path).write_text(
        f"noise\n{BUILD_MARKER} source={TIMED_SOURCE} compiling=2\ntrailing\n",
        encoding="utf-8",
    )
    reports = read_build_reports(tmp_path)
    assert [(r.source, r.compiling) for r in reports] == [(TIMED_SOURCE, 2)], (
        f"unrelated lines changed the reading: {reports}"
    )
