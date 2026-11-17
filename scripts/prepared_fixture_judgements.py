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
compilation: nextest suppresses a passing test's output, and the crate's own
helper returns the child's stderr only to discard it on success. Both carriers
this module could have read are empty by construction, so the count arrives
through the file ``read_build_reports`` reads instead.

Two facts reach that file, and the reuse check needs both. The gate files a
zero when it admits a stamp, which proves no *preparation* ran. The timed
command files its own count from the stderr it captures before discarding it,
which is the only evidence about the timed run itself. An earlier version of
this check read the gate's zero alone and reported it as reuse; that was true
and beside the point, because the command it was supposed to be judging had not
run yet.
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


def judge_run_counts(
    target_dir: Path,
    previous_counts: int,
    combined: str,
) -> tuple[list[int], list[int]]:
    """Split the report into this run's counts and the counts before it.

    The gate's report file accumulates a line per run, so the counts written
    during the run under judgement have to be told apart from the lines already
    there. A run that added nothing is a failure rather than a pass: it means
    the gate never wrote a report, so nothing vouches for what the timed command
    compiled.

    Parameters
    ----------
    target_dir : Path
        The private target directory, which holds the report.
    previous_counts : int
        How many report lines existed before this run.
    combined : str
        The run's stdout and stderr, used for the failure message.

    Returns
    -------
    tuple[list[int], list[int]]
        The counts this run appended, then the counts that predate it. Both
        halves appear in the caller's failure messages.

    Raises
    ------
    StepFailureError
        If the run appended no report line at all.
    """
    counts = read_build_reports(target_dir)
    prior = counts[:previous_counts]
    this_run = counts[previous_counts:]
    if not this_run:
        msg = (
            "the timed test left no nested-build report, so nothing shows "
            "whether it reused the prepared tree. The gate writes one to "
            f"${BUILD_REPORT_ENV} on every run, so its absence means the gate "
            f"never ran. Counts before this run: {prior}; "
            f"after: {counts}. Output:\n{combined}"
        )
        raise StepFailureError(msg)
    return this_run, prior


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

    Two counts arrive per run and both must be zero, because they answer
    different questions. The gate's ``record_reuse`` reports that no
    *preparation* ran, which by itself says nothing about the timed command: it
    runs afterwards, in the test body. The timed command files its own count
    from the stderr it captures, which is the only witness to a rebuild there —
    its caller discards that stderr on success, so a scan of the run's output
    would find nothing however much was compiled. Judging the pair is what
    keeps "the gate skipped preparation" from passing in place of "the timed
    command compiled nothing".

    The counts are read from the report file rather than from the step's output
    precisely because the output cannot carry them. See [`read_build_reports`].

    Only the lines *this* run added are judged, not the whole file. The file
    accumulates lines across gate runs, and step 2's cold preparation
    legitimately compiles tens of units and has its count written at the end of
    that run; comparing the whole history would fail this step for the
    preparation that is supposed to precede it. The counts read *before* the run
    are passed in so the lines written during it can be told apart from the
    lines already there.

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
    this_run, prior = judge_run_counts(target_dir, previous_counts, combined)
    rebuilt = [count for count in this_run if count != 0]
    if rebuilt:
        msg = (
            "the timed test's nested build compiled after preparation, so the "
            f"prepared tree was not reused: this run reported {this_run} "
            f"(the preparation's own count was {prior})"
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
