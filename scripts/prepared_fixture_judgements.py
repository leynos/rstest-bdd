"""What each step of the prepared-fixture workflow must mean.

The driver in ``check_prepared_fixture_workflow`` starts the steps; this module
decides whether their outcomes are what the workflow requires. The two are
separated because only one of them can be tested cheaply. Starting a step needs
a cold Cargo preparation of a whole fixture family — minutes on a warm cache and
more on CI — so a judgement embedded in a step is exercised only when someone
affords the whole run. An earlier version of that check could never fail, and
nothing noticed, because its defects were invisible at this price.

Every function here therefore takes an outcome that has *already happened* and
answers about it, which is what lets each one be checked in both directions by
the cases in ``tests/test_check_prepared_fixture_workflow.py``.

The evidence channel deserves a word, because it is the reason the reuse check
is not a second scan of stdout. A step's captured output cannot show a nested
compilation: the helper which spawns ``cargo bdd steps`` discards its capture on
success, and nextest suppresses a passing test's output. The two carriers this
module could have read are both empty by construction, so the count arrives
through the file ``read_build_reports`` reads instead.
"""

import typing as typ

from prepared_fixture_step import (
    BUILD_REPORT_ENV,
    REMEDY,
    TIMED_TEST,
    read_build_reports,
)

if typ.TYPE_CHECKING:
    import subprocess
    from pathlib import Path

__all__ = [
    "StepFailureError",
    "expect_pass_without_rebuild",
    "expect_preparation",
    "expect_refusal",
    "expect_stale_stamp_refusal",
]


class StepFailureError(Exception):
    """A workflow step produced the wrong outcome."""


def expect_refusal(result: subprocess.CompletedProcess[str]) -> None:
    """Require *result* to be the unprepared-tree refusal.

    Parameters
    ----------
    result : subprocess.CompletedProcess[str]
        The first run of the timed test.

    Raises
    ------
    StepFailureError
        If the run passed, or failed without naming the remedy.
    """
    combined = f"{result.stdout}\n{result.stderr}"
    if "no tests to run" in combined:
        # A drifted filter is not a refusal. Treating its non-zero status as
        # one is the exact way this gate could pass while testing nothing.
        msg = (
            "the filter selected no tests (`no tests to run`), so a drifted "
            f"filter would masquerade as a refusal. Filter: {TIMED_TEST}"
        )
        raise StepFailureError(msg)
    if result.returncode == 0:
        msg = (
            "the timed test passed against an empty target directory; the "
            "preparation gate is not refusing an unprepared tree, so steps 2 "
            "and 3 would prove nothing"
        )
        raise StepFailureError(msg)
    if REMEDY not in combined:
        msg = (
            f"the timed test refused, but did not name `{REMEDY}`; a reader "
            f"would be left without the remedy. Output:\n{combined}"
        )
        raise StepFailureError(msg)


def expect_pass_without_rebuild(
    result: subprocess.CompletedProcess[str],
    *,
    target_dir: Path,
    previous_counts: int,
) -> None:
    """Require *result* to pass and to report no nested compilation.

    The second half matters as much as the first. A pass alone is also what a
    warm shared cache produces; requiring a reported count of zero is what
    shows preparation built *this* target directory rather than the run finding
    the graph warm by accident.

    The count is read from the report file rather than from the step's output
    precisely because the output cannot carry it. See [`read_build_reports`].

    Only *this* step's line is judged, not the whole file. The file accumulates
    one line per gate run, and step 2's cold preparation legitimately compiles
    tens of units and has its count written at the end of that run; comparing
    the whole history would fail this step for the preparation that is supposed
    to precede it. The counts read *before* the run are passed in so the lines
    written during it can be told apart from the lines already there.

    Parameters
    ----------
    result : subprocess.CompletedProcess[str]
        The second run of the timed test.
    target_dir : Path
        The private target directory, which holds the report.
    previous_counts : int
        How many report lines existed before this run, so the lines it added
        can be identified rather than inferred from their value.

    Raises
    ------
    StepFailureError
        If the run failed, reported no build at all, or compiled anything on
        the way.
    """
    combined = f"{result.stdout}\n{result.stderr}"
    if result.returncode != 0:
        msg = (
            "the timed test failed after `make prepare-nested-fixtures`, so "
            "the remedy does not admit the test it exists for. Output:\n"
            f"{combined}"
        )
        raise StepFailureError(msg)
    counts = read_build_reports(target_dir)
    this_run = counts[previous_counts:]
    if not this_run:
        msg = (
            "the timed test left no nested-build report, so nothing shows "
            "whether it reused the prepared tree. The gate writes one to "
            f"${BUILD_REPORT_ENV} on every run, so its absence means the gate "
            f"never ran. Counts before this run: {counts[:previous_counts]}; "
            f"after: {counts}. Output:\n{combined}"
        )
        raise StepFailureError(msg)
    rebuilt = [count for count in this_run if count != 0]
    if rebuilt:
        msg = (
            "the timed test's nested build compiled after preparation, so the "
            f"prepared tree was not reused: this run reported {this_run} "
            f"(the preparation's own count was {counts[:previous_counts]})"
        )
        raise StepFailureError(msg)


def expect_preparation(result: subprocess.CompletedProcess[str]) -> None:
    """Require the remedy run to have succeeded.

    Parameters
    ----------
    result : subprocess.CompletedProcess[str]
        The run of ``make prepare-nested-fixtures``.

    Raises
    ------
    StepFailureError
        If the preparation exited non-zero.
    """
    if result.returncode != 0:
        msg = (
            "`make prepare-nested-fixtures` failed against the private "
            f"target directory. Output:\n{result.stdout}\n{result.stderr}"
        )
        raise StepFailureError(msg)


def expect_stale_stamp_refusal(result: subprocess.CompletedProcess[str]) -> None:
    """Require *result* to be a refusal caused by the stamp, not anything else.

    Split from its caller so the judgement can be unit-tested without spawning
    a step: the caller owns the mutation and its restore, and this owns what
    the outcome must mean.

    Parameters
    ----------
    result : subprocess.CompletedProcess[str]
        The timed run made against the edited fixture source.

    Raises
    ------
    StepFailureError
        If the run passed — the stamp survived an input change — or failed
        without naming the remedy, which makes it some other failure.
    """
    combined = f"{result.stdout}\n{result.stderr}"
    if result.returncode == 0:
        msg = (
            "the timed test passed after its fixture source was edited, so the "
            "preparation stamp survived an input change and the no-rebuild "
            f"check cannot distinguish a prepared tree from a stale one. "
            f"Output:\n{combined}"
        )
        raise StepFailureError(msg)
    if REMEDY not in combined:
        msg = (
            "the timed test failed after its fixture source was edited but did "
            f"not name `{REMEDY}`, so the failure was not the stamp refusing a "
            f"stale tree. Output:\n{combined}"
        )
        raise StepFailureError(msg)
