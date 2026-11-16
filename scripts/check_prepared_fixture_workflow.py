#!/usr/bin/env python3
"""Prove the filtered-``nextest`` preparation workflow end to end.

``make prepare-nested-fixtures`` and the ``nested_fixture_preparation``
override are two halves of one policy, and until this script nothing exercised
them together. The pieces that existed each miss the point:

- ``scripts/tests/test_prepare_nested_fixtures_makefile_contract.py`` pins the
  recipe's text, which catches a drifted filter but never runs it.
- ``nested_cargo``'s in-process sequence tests drive the stamp and the gate,
  but inside one process against a scratch tree, so no process boundary and no
  Make target is involved.
- The hosted lanes run the target, but only inside a lane that has already
  prepared everything.

The gap those leave is the workflow itself: a timed test must *fail* on an
unprepared tree with a message naming the remedy, and must *pass* after the
remedy runs. This script drives exactly that, as three separate commands in
this process so there is no ``nextest`` inside ``nextest``:

1. Run the timed test against an empty private target directory and require
   the refusal, checking the message names ``make prepare-nested-fixtures``.
2. Run ``make prepare-nested-fixtures`` against the same directory.
3. Run the same timed test again and require it to pass, and require that its
   output reports no compilation so a rebuild cannot be mistaken for a pass.

The private target directory is what makes step 1 meaningful: the workspace
``target/`` is shared with the rest of a lane and is warm, so a timed test
there would pass for a reason unrelated to preparation. It is also what keeps
the three steps from disturbing any other job.

Steps 1 and 3 go through ``make nextest-run``, not a bare ``cargo nextest
run``. That is not a stylistic choice. The stamp fingerprints ``RUSTFLAGS``,
among other build-relevant variables, and ``prepare-nested-fixtures`` pins it
to ``-D warnings -Clink-arg=-fuse-ld=mold``. A hand-typed ``cargo nextest
run`` inherits ``RUSTFLAGS`` from the caller — unset in an ordinary shell — so
it computes a different fingerprint and refuses the stamp the preparation has
just written. Both halves are correct and still do not compose, which is the
defect this script found the first time it ran end to end: step 3 reproduced
step 1's refusal verbatim. Routing both halves through the Makefile gives the
build environment one owner instead of a Python copy of it.

This is deliberately *not* part of ``make test``. A cold preparation of this
family was measured at 89.382 s on a warm cache and 239.213 s on CI, which is
too expensive for every local run; it belongs in its own CI job.

Usage: ``python3 scripts/check_prepared_fixture_workflow.py [--keep]``.

Exit codes: 0 when all three steps hold, 1 when any step does not. ``--keep``
leaves the private target directory in place for inspection.
"""

import argparse
import os
import shutil
import subprocess  # ruff: ignore[suspicious-subprocess-import] - runs fixed argvs built above; no shell.
import sys
import tempfile
import typing as typ
from pathlib import Path

#: The repository root, two levels above ``scripts/``.
REPO_ROOT = Path(__file__).resolve().parents[1]

#: The timed test the CLI family gates on preparation.
#:
#: Chosen because it is cheap, it is a real user-visible command
#: (``cargo bdd steps``), and its binary also holds the family's
#: ``nested_fixture_preparation`` test, so the target and the timed test share
#: one filtered suite.
#:
#: The filter is binary-qualified and uses nextest's *exact* test-name
#: operator. Both parts are load-bearing. A ``test(/.../)`` regex is matched
#: against the *test name*, not the ``<binary>::<test>`` path nextest prints,
#: so a filter written as ``test(/cargo-bdd::cli::list_steps_runs$/)`` matches
#: nothing at all; nextest then exits non-zero with ``no tests to run``, which
#: a driver reading only the exit status would score as the expected refusal,
#: and the whole gate would "pass" while running no test. This is the same
#: empty-filter hazard the Makefile contract test exists for.
#:
#: ``test(=name)`` is also why this string survives Make. Make expands ``$``
#: in a command-line variable, so an anchored ``test(/^list_steps_runs$/)``
#: would reach nextest as ``test(/^list_steps_runs/)`` unless escaped as
#: ``$$``. Exact match needs no anchor and so needs no escaping.
TIMED_TEST = "binary_id(cargo-bdd::cli) & test(=list_steps_runs)"

#: The remedy the refusal must name, so a reader is told what to run.
REMEDY = "make prepare-nested-fixtures"

#: The run half of the same policy, which owns the build environment.
#:
#: A bare ``cargo nextest run`` here would carry the caller's ``RUSTFLAGS``
#: rather than the preparation's, and refuse the stamp step 2 just wrote.
RUNNER_TARGET = "nextest-run"

#: Bound each step so a hung nested build fails the gate instead of the job.
STEP_TIMEOUT_SECONDS = 1800


class StepFailureError(Exception):
    """A workflow step produced the wrong outcome."""


def run_step(
    argv: list[str],
    *,
    target_dir: Path,
) -> subprocess.CompletedProcess[str]:
    """Run one workflow command against *target_dir* and return its result.

    Only ``CARGO_TARGET_DIR`` is set here. The build-relevant environment —
    ``RUSTFLAGS`` above all — is the Makefile's to own: this script and the
    targets it drives must not diverge on it, which is the failure that made
    step 3 reproduce step 1.

    Parameters
    ----------
    argv : list[str]
        The command and its arguments.
    target_dir : Path
        The private target directory every step shares.

    Returns
    -------
    subprocess.CompletedProcess[str]
        The captured result, never raised on a non-zero status: each step
        decides for itself what an acceptable outcome is.
    """
    env = os.environ.copy()
    env["CARGO_TARGET_DIR"] = str(target_dir)
    return subprocess.run(  # ruff: ignore[subprocess-without-shell-equals-true] - fixed argv, no shell.
        argv,
        cwd=REPO_ROOT,
        env=env,
        capture_output=True,
        text=True,
        timeout=STEP_TIMEOUT_SECONDS,
        check=False,
    )


def timed_test_argv() -> list[str]:
    """Return the argv that runs the timed test through the Makefile."""
    return ["make", RUNNER_TARGET, f"EXPR={TIMED_TEST}"]


def preparation_argv() -> list[str]:
    """Return the argv that runs the documented remedy."""
    return ["make", "prepare-nested-fixtures"]


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


def expect_pass_without_rebuild(result: subprocess.CompletedProcess[str]) -> None:
    """Require *result* to pass and to report no compilation.

    The second half matters as much as the first. A pass alone is also what a
    warm shared cache produces; requiring zero ``Compiling`` lines is what
    shows step 2 prepared *this* target directory rather than step 3 finding
    the graph warm by accident.

    Parameters
    ----------
    result : subprocess.CompletedProcess[str]
        The second run of the timed test.

    Raises
    ------
    StepFailureError
        If the run failed, or compiled anything on the way.
    """
    combined = f"{result.stdout}\n{result.stderr}"
    if result.returncode != 0:
        msg = (
            "the timed test failed after `make prepare-nested-fixtures`, so "
            "the remedy does not admit the test it exists for. Output:\n"
            f"{combined}"
        )
        raise StepFailureError(msg)
    compiling = [line for line in combined.splitlines() if "Compiling" in line]
    if compiling:
        msg = (
            "the timed test compiled after preparation, so the prepared tree "
            f"was not reused: {compiling[:5]}"
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


def run_workflow(target_dir: Path) -> None:
    """Drive the three steps against *target_dir*.

    Parameters
    ----------
    target_dir : Path
        The private target directory every step shares.

    Notes
    -----
    Anything a step raises propagates to the caller, which is what turns it
    into this script's exit status and its diagnostic line.
    """
    report(sys.stderr, "step 1/3: the timed test must refuse an unprepared tree")
    expect_refusal(run_step(timed_test_argv(), target_dir=target_dir))

    report(sys.stderr, "step 2/3: run the documented remedy")
    expect_preparation(run_step(preparation_argv(), target_dir=target_dir))

    report(sys.stderr, "step 3/3: the same timed test must now pass")
    expect_pass_without_rebuild(run_step(timed_test_argv(), target_dir=target_dir))


def report(stream: typ.TextIO, message: str) -> None:
    """Write one progress line to *stream*."""
    print(message, file=stream, flush=True)


def main(argv: list[str] | None = None) -> int:
    """Drive the three workflow steps and report the outcome.

    Parameters
    ----------
    argv : list[str] | None
        Command-line arguments, or None to read ``sys.argv``.

    Returns
    -------
    int
        Zero when every step held, one otherwise.
    """
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--keep",
        action="store_true",
        help="leave the private target directory in place for inspection",
    )
    args = parser.parse_args(argv)

    target_dir = Path(tempfile.mkdtemp(prefix="prepared-fixture-workflow-"))
    report(sys.stderr, f"private target directory: {target_dir}")
    try:
        run_workflow(target_dir)
    except StepFailureError as error:
        report(sys.stderr, f"FAILED: {error}")
        return 1
    except subprocess.TimeoutExpired as error:
        report(
            sys.stderr,
            f"FAILED: a step exceeded {STEP_TIMEOUT_SECONDS} s: {error}",
        )
        return 1
    finally:
        if not args.keep:
            shutil.rmtree(target_dir, ignore_errors=True)
        else:
            report(sys.stderr, f"kept {target_dir}")

    report(sys.stderr, "the filtered preparation workflow holds end to end")
    return 0


if __name__ == "__main__":
    sys.exit(main())
