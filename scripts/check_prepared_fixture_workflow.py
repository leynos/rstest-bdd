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
remedy runs. This script drives exactly that, as four separate commands in
this process so there is no ``nextest`` inside ``nextest``:

1. Run the timed test against an empty private target directory and require
   the refusal, checking the message names ``make prepare-nested-fixtures``.
2. Run ``make prepare-nested-fixtures`` against the same directory.
3. Run the same timed test again and require it to pass, and require the
   nested-build report to show it compiled nothing *and* to hold one line from
   each producer — the gate's and the timed command's — so a rebuild cannot be
   mistaken for a pass, and neither can a run that only reached one of them.
4. Mutate the fixture source and require the next run to refuse, naming the
   remedy. Without this the third step's check could be a predicate that never
   fails, and a check that cannot fail cannot vouch for anything. The stamp
   hashes the fixture inputs, so an edit must invalidate it: the zero in step 3
   is a reused prepared tree, not an unconditional constant.

The report step 3 reads is a *file*, named by ``RSTEST_BDD_NESTED_BUILD_REPORT``
and written by the fixture's gate, rather than a line in the step's output. The
first version of this script scanned the captured output for ``Compiling``
lines, and that scan could never have worked: the helper which spawns
``cargo bdd steps`` discards its capture on success, and nextest suppresses a
passing test's output, so the nested compilation reached neither stream. The
scan found nothing whatever the build did and reported reuse. A report on a
channel both carriers drop is the same defect as no check at all.

The private target directory is what makes step 1 meaningful: the workspace
``target/`` is shared with the rest of a lane and is warm, so a timed test
there would pass for a reason unrelated to preparation. It is also what keeps
the steps from disturbing any other job, and it is where the nested-build
report lives so the evidence is removed along with the tree it describes.

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

Starting a step, bounding it, and reading its report live in
``prepared_fixture_step``; this module composes the steps and decides what
each outcome must be.

Usage: ``python3 scripts/check_prepared_fixture_workflow.py [--keep]``.

Exit codes: 0 when all four steps hold, 1 when any step does not. ``--keep``
leaves the private target directory in place for inspection.
The directory is also kept when a timed-out step's processes cannot be
confirmed gone, because they may still be writing into it.
"""

import argparse
import shutil
import subprocess  # ruff: ignore[suspicious-subprocess-import] - only catches TimeoutExpired.
import sys
import tempfile
import typing as typ
from pathlib import Path

from prepared_fixture_judgements import (
    StepFailureError,
    expect_pass_without_rebuild,
    expect_preparation,
    expect_refusal,
    expect_stale_stamp_refusal,
)
from prepared_fixture_step import (
    REMEDY,
    REPO_ROOT,
    STEP_TIMEOUT_SECONDS,
    TreeSurvivedError,
    build_report_path,
    preparation_argv,
    read_build_reports,
    run_step,
    timed_test_argv,
)

__all__ = [
    "REMEDY",
    "StepFailureError",
    "build_report_path",
    "expect_mutation_invalidates_the_stamp",
    "expect_pass_without_rebuild",
    "expect_preparation",
    "expect_refusal",
    "expect_stale_stamp_refusal",
    "main",
    "read_build_reports",
    "run_workflow",
]

#: Where the stamp-invalidation control edits a fixture source.
FIXTURE_SOURCE = "crates/cargo-bdd/tests/fixtures/minimal/src/lib.rs"

#: The bytes that control adds, and then removes again.
#:
#: A comment line, chosen because it changes the file's hash — which is what the
#: stamp records — while changing nothing the build compiles to. So it can only
#: be detected through the input hash, which is exactly the mechanism under
#: test.
NEGATIVE_CONTROL_EDIT = b"\n// Negative control for the prepared-fixture workflow.\n"


def expect_mutation_invalidates_the_stamp(target_dir: Path) -> None:
    """Prove the stamp is load-bearing, not merely present.

    [`expect_pass_without_rebuild`] reports success when a count is zero, and
    the defect it was written to catch was that it could not distinguish zero
    from silence. A positive run on an already-prepared tree cannot tell a
    working check from one that always passes, so this mutates a fixture source
    and requires the next timed run to *refuse*. That is the whole mechanism:
    the stamp hashes the fixture inputs, so a source edit must invalidate it.

    An earlier version of this step demanded a *compile* instead. That could
    never happen. Once the stamp is admitted the gate runs no preparation at
    all, and once it is stale the gate refuses rather than preparing — the
    build happens on the remedy path, never on the timed one. Asking the timed
    run for a nonzero count asked for a state the design forbids.

    The refusal also has to name the remedy, which proves the refusal is the
    stamp's rather than any other failure wearing the same exit status.

    The original bytes are restored in a ``finally``, so a failed assertion
    here does not leave a dirty working tree behind; the restore is the reason
    this is not a plain ``assert`` after the rewrite.

    Parameters
    ----------
    target_dir : Path
        The private target directory the mutated run is driven against.

    Notes
    -----
    The judgement of the outcome is delegated, so the failure a caller sees is
    raised by [`expect_stale_stamp_refusal`]: either the mutated run passed,
    meaning the stamp survived an input change, or it failed without naming the
    remedy, meaning the failure was something else.
    """
    fixture = REPO_ROOT / FIXTURE_SOURCE
    original = fixture.read_bytes()
    try:
        fixture.write_bytes(original + NEGATIVE_CONTROL_EDIT)
        expect_stale_stamp_refusal(run_step(timed_test_argv(), target_dir=target_dir))
    finally:
        fixture.write_bytes(original)


def run_workflow(target_dir: Path) -> None:
    """Drive the workflow's steps against *target_dir*.

    Parameters
    ----------
    target_dir : Path
        The private target directory every step shares.

    Notes
    -----
    Anything a step raises propagates to the caller, which is what turns it
    into this script's exit status and its diagnostic line.
    """
    report(sys.stderr, "step 1/4: the timed test must refuse an unprepared tree")
    expect_refusal(run_step(timed_test_argv(), target_dir=target_dir))

    report(sys.stderr, "step 2/4: run the documented remedy")
    expect_preparation(run_step(preparation_argv(), target_dir=target_dir))

    report(sys.stderr, "step 3/4: the same timed test must now pass")
    previous_counts = len(read_build_reports(target_dir))
    expect_pass_without_rebuild(
        run_step(timed_test_argv(), target_dir=target_dir),
        target_dir=target_dir,
        previous_counts=previous_counts,
    )

    report(sys.stderr, "step 4/4: an edited fixture must invalidate the stamp")
    expect_mutation_invalidates_the_stamp(target_dir)


def report(stream: typ.TextIO, message: str) -> None:
    """Write one progress line to *stream*."""
    print(message, file=stream, flush=True)


def main(argv: list[str] | None = None) -> int:
    """Drive the four workflow steps and report the outcome.

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
    keep = args.keep
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
    except TreeSurvivedError as error:
        # The step's processes may still be writing into the directory, so
        # removing it would only move the failure somewhere harder to read.
        keep = True
        report(
            sys.stderr,
            f"FAILED: a step exceeded {STEP_TIMEOUT_SECONDS} s and its "
            f"processes could not be confirmed gone: {error}",
        )
        return 1
    finally:
        if not keep:
            shutil.rmtree(target_dir, ignore_errors=True)
        else:
            report(sys.stderr, f"kept {target_dir}")

    report(sys.stderr, "the filtered preparation workflow holds end to end")
    return 0


if __name__ == "__main__":
    sys.exit(main())
