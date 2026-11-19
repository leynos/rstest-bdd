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

Both reports are therefore required from ``expect_pass_without_rebuild``, each
exactly once, not merely counted. A run that stopped, panicked or never reached
the command leaves the gate's zero behind and nothing else, and an earlier
version of that check read exactly that as reuse; a run whose gate never filed
anything can equally leave the timed line alone. Each line names its producer,
so a run missing either one fails however many lines it filed, and a run filing
two from one producer fails as ambiguous.

Only the lines a run *adds* are judged, not the whole file. The file
accumulates lines across gate runs, and the cold preparation legitimately
compiles tens of units and has its count written at the end of its own run;
comparing the whole history would fail the reuse step for the preparation that
is supposed to precede it. The count read *before* the run is passed in so the
lines written during it can be told apart from the lines already there.
"""

import collections
import typing as typ

from prepared_fixture_step import (
    BUILD_REPORT_ENV,
    REMEDY,
    REQUIRED_SOURCES,
    TIMED_TEST,
    read_build_reports,
)

if typ.TYPE_CHECKING:
    import subprocess
    from pathlib import Path

    from prepared_fixture_step import BuildReport

__all__ = [
    "StepFailureError",
    "expect_pass_without_rebuild",
    "expect_preparation",
    "expect_refusal",
    "expect_stale_stamp_refusal",
    "require_one_of_each",
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
    drifted = (
        "the filter selected no tests (`no tests to run`), so a drifted "
        f"filter would masquerade as a refusal. Filter: {TIMED_TEST}"
    )
    admitted = (
        "the timed test passed against an empty target directory; the "
        "preparation gate is not refusing an unprepared tree, so steps 2 and 3 "
        "would prove nothing"
    )
    unnamed = (
        f"the timed test refused, but did not name `{REMEDY}`; a reader would "
        f"be left without the remedy. Output:\n{combined}"
    )
    # Checked in this order: a drifted filter first, because treating its
    # non-zero status as a refusal is the exact way this gate could pass while
    # testing nothing; then a pass, which means the gate admitted an empty tree;
    # then a failure that does not name the remedy, which is some other failure.
    checks = (
        ("no tests to run" in combined, drifted),
        (result.returncode == 0, admitted),
        (REMEDY not in combined, unnamed),
    )
    for failed, message in checks:
        if failed:
            raise StepFailureError(message)


def judge_run_reports(
    target_dir: Path,
    previous_counts: int,
    combined: str,
) -> tuple[list[BuildReport], list[BuildReport]]:
    """Split the report into this run's entries and the entries before it.

    The report file accumulates a line per run, so what was written during the
    run under judgement has to be told apart from what was already there. Lines
    are attributed by *position* — the file is append-only and the caller
    snapshots its length — rather than by value, because a count of zero is a
    legitimate result for two of the three producers.

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
    tuple[list[BuildReport], list[BuildReport]]
        The lines this run appended, then the lines that predate it. Both
        halves appear in the caller's failure messages.

    Raises
    ------
    StepFailureError
        If reading the report fails.
    """
    try:
        reports = read_build_reports(target_dir)
    except OSError as error:
        msg = (
            "the nested-build report could not be read, so nothing shows "
            f"whether the timed test reused the prepared tree: {error}. "
            f"It is named by ${BUILD_REPORT_ENV}. Output:\n{combined}"
        )
        raise StepFailureError(msg) from error
    return reports[previous_counts:], reports[:previous_counts]


def require_one_of_each(
    this_run: list[BuildReport],
    *,
    previous_counts: int,
    combined: str,
) -> None:
    """Require exactly one report from each producer the reuse check needs.

    A passing reuse run has to show two different facts, and each is filed by
    its own producer: the gate files one line when it admits a stamp, and the
    timed command files one line after it has paid for its own nested build.
    Either line on its own leaves the other fact unproven — the gate's zero is
    what a run that stopped before the test body leaves behind, and a timed
    zero with no gate line is a run whose gate never reported at all. Earlier
    versions of this check accepted one line of either kind, so a missing or
    malformed line could satisfy it.

    Duplicates are rejected as well as absences. Two lines from one producer
    mean the run did the thing twice, or that a stale line has been attributed
    to this run, and neither state is the one the workflow asserts. Requiring
    exactly one keeps the answer to "did *this* run reuse the tree" a single
    fact per producer rather than a value that depends on how many lines were
    written.

    Parameters
    ----------
    this_run : list[BuildReport]
        The reports this run appended, in the order they were written.
    previous_counts : int
        How many report lines existed before this run, used only to make the
        failure message say what the file held before it.
    combined : str
        The run's stdout and stderr, used for the failure message.

    Raises
    ------
    StepFailureError
        If any required producer filed no line, or more than one.
    """
    counts = collections.Counter(report.source for report in this_run)
    problems = [
        describe_count_offence(source, counts[source])
        for source in REQUIRED_SOURCES
        if counts[source] != 1
    ]
    if problems:
        msg = (
            f"{' '.join(problems)} The reuse check needs exactly one line from "
            f"each of {list(REQUIRED_SOURCES)}; this run filed {this_run} (the "
            f"preparation's own reports were the {previous_counts} lines "
            f"before it). Output:\n{combined}"
        )
        raise StepFailureError(msg)


def describe_count_offence(source: str, filed: int) -> str:
    """Say how `source`'s report count departs from the required one.

    Parameters
    ----------
    source : str
        The producer the offending report should have come from.
    filed : int
        How many reports that producer filed this run.

    Returns
    -------
    str
        One clause naming the producer and what it filed: nothing, or more
        than one. Callers have already established that `filed` is not one.
    """
    if filed == 0:
        return (
            f"This run filed no `source={source}` nested-build report, so it "
            "cannot show the fact that producer witnesses."
        )
    return (
        f"This run filed {filed} `source={source}` nested-build reports, so "
        "its evidence is ambiguous: exactly one line is required from each "
        "producer."
    )


def expect_pass_without_rebuild(
    result: subprocess.CompletedProcess[str],
    *,
    target_dir: Path,
    previous_counts: int,
) -> None:
    """Require *result* to pass and to report no nested compilation.

    A pass alone is what a warm shared cache also produces; requiring a
    reported count of zero is what shows preparation built *this* target
    directory rather than the run finding the graph warm by accident. Both
    producers must have filed exactly one report, each with a zero count. See
    the module docstring for why both, and why only this run's lines are
    judged.

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
            f"the remedy does not admit the test it exists for. Output:\n{combined}"
        )
        raise StepFailureError(msg)
    this_run, prior = judge_run_reports(target_dir, previous_counts, combined)
    # Nothing at all is a different failure from a run that filed one producer
    # and not the other: it is the absence the report file exists to make
    # visible, so it is named separately.
    if not this_run:
        msg = (
            "the timed test left no nested-build report, so nothing shows "
            "whether it reused the prepared tree. The gate writes one to "
            f"${BUILD_REPORT_ENV} on every run, so its absence means the gate "
            f"never ran. Reports before this run: {prior}. Output:\n{combined}"
        )
        raise StepFailureError(msg)
    require_one_of_each(this_run, previous_counts=previous_counts, combined=combined)
    require_no_rebuild(this_run, prior)


def require_no_rebuild(this_run: list[BuildReport], prior: list[BuildReport]) -> None:
    """Require every report this run filed to record zero compiled units.

    Parameters
    ----------
    this_run : list[BuildReport]
        The reports this run appended.
    prior : list[BuildReport]
        The reports that existed before this run, used only for the message.

    Raises
    ------
    StepFailureError
        If any report this run filed counted a compilation.
    """
    rebuilt = [report.compiling for report in this_run if report.compiling != 0]
    if rebuilt:
        msg = (
            "the timed test's nested build compiled after preparation, so the "
            f"prepared tree was not reused: the timed command reported {rebuilt} "
            f"compiled units (this run's reports were {this_run}, the "
            f"preparation's own were {prior})"
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
    survived = (
        "the timed test passed after its fixture source was edited, so the "
        "preparation stamp survived an input change and the no-rebuild check "
        f"cannot distinguish a prepared tree from a stale one. Output:\n{combined}"
    )
    other_failure = (
        "the timed test failed after its fixture source was edited but did not "
        f"name `{REMEDY}`, so the failure was not the stamp refusing a stale "
        f"tree. Output:\n{combined}"
    )
    # A pass means the stamp survived an input change; a failure that does not
    # name the remedy is some other failure. Either way the step has not shown
    # what it exists to show.
    checks = (
        (result.returncode == 0, survived),
        (REMEDY not in combined, other_failure),
    )
    for failed, message in checks:
        if failed:
            raise StepFailureError(message)
