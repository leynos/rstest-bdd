"""Contract for how the publish step's compiler-cache report is handled.

The publish dry run asks lading for a report; a verification step reads
it and says when there is none; an upload collects it. This module
asserts the shape of those three steps and the budgets around them: that
the paths agree, that the order is write, read, collect, that both
report steps run on a failed Linux run, and that the artefact is named
per lane and kept.

Running the verification script for real is
:mod:`publish_verification_script_test`; this module reads the workflow.

Run via ``make test-workflow-contracts``.
"""

import typing as typ

import pytest
from publish_report_support import (
    DRY_RUN_STEP,
    LINUX_ALWAYS,
    RETENTION_DAYS,
    STATS_VARIABLE,
    UPLOAD_ACTION,
    UPLOAD_STEP,
    VERIFY_STEP,
    dry_run_steps,
    mapping_at,
    statistics_path,
    step_index,
    step_named,
)


def test_the_dry_run_asks_for_compiler_cache_statistics(
    build_test_job: dict[str, typ.Any],
) -> None:
    """The publish step's cost must stay attributable.

    Without this the step is the longest in the job and says nothing
    about what it spent, which is what made the earlier 36-minute runs
    impossible to reason about.
    """
    steps = dry_run_steps(build_test_job)
    assert steps, f"ci.yml must have a {DRY_RUN_STEP!r} step"

    for step in steps:
        environment = mapping_at(step, "env", f"the {DRY_RUN_STEP!r} step")
        assert STATS_VARIABLE in environment, (
            f"the {DRY_RUN_STEP!r} step must set {STATS_VARIABLE} so lading "
            f"reports the compiler cache around each packaged build"
        )
        assert str(environment[STATS_VARIABLE]).endswith(".json"), (
            f"{STATS_VARIABLE} must name a JSON file, got "
            f"{environment[STATS_VARIABLE]!r}"
        )


def test_the_statistics_file_is_uploaded(build_test_job: dict[str, typ.Any]) -> None:
    """A file written into RUNNER_TEMP and never uploaded is not evidence.

    The upload has to collect the path the publish step wrote, so the two
    are compared rather than each checked against a constant. A step that
    uploaded a stale path would satisfy either check alone.
    """
    upload = step_named(build_test_job, UPLOAD_STEP)
    assert UPLOAD_ACTION in str(upload.get("uses", "")), (
        f"{UPLOAD_STEP!r} must use {UPLOAD_ACTION}, not {upload.get('uses')!r}"
    )
    with_block = mapping_at(upload, "with", f"the {UPLOAD_STEP!r} step")
    assert with_block.get("path") == statistics_path(build_test_job), (
        f"{UPLOAD_STEP!r} uploads {with_block.get('path')!r}, which is not "
        f"the {statistics_path(build_test_job)!r} the publish step writes; the report "
        f"would be discarded with the runner"
    )


def test_the_upload_follows_the_publishstep_named(
    build_test_job: dict[str, typ.Any],
) -> None:
    """Ordering is the whole of it: nothing exists to upload before.

    A step order that put the upload first would pass every assertion
    about its inputs and collect nothing on every run.
    """
    publish = step_index(build_test_job, DRY_RUN_STEP)
    verify = step_index(build_test_job, VERIFY_STEP)
    upload = step_index(build_test_job, UPLOAD_STEP)
    assert publish < verify < upload, (
        f"the publish step (index {publish}) must precede the verification "
        f"(index {verify}) and the upload (index {upload}); a report is "
        f"read and collected after it is written, not before"
    )


@pytest.mark.parametrize("step_name", [VERIFY_STEP, UPLOAD_STEP], ids=str)
def test_the_report_steps_run_on_a_failed_linux_run(
    build_test_job: dict[str, typ.Any], step_name: str
) -> None:
    """The run worth reading is often the one that failed.

    Both halves of the condition are load-bearing and each fails a
    different way. Without ``always()`` a failed publish uploads nothing,
    which is the run whose cost is worth knowing. Without the Linux
    guard, the Windows lanes, which never write the file, would report a
    missing one on every run.
    """
    condition = str(step_named(build_test_job, step_name).get("if", ""))
    assert condition == LINUX_ALWAYS, (
        f"{step_name!r} has if: {condition!r}, not {LINUX_ALWAYS!r}"
    )


def test_the_artefact_is_named_per_lane_and_kept(
    build_test_job: dict[str, typ.Any],
) -> None:
    """One name per lane, kept long enough to compare two runs.

    A name shared across the matrix collides, and the second lane's
    upload fails or overwrites the first. A retention shorter than the
    comparison window makes the artefact evidence that expires before it
    is read.
    """
    with_block = mapping_at(
        step_named(build_test_job, UPLOAD_STEP), "with", f"the {UPLOAD_STEP!r} step"
    )
    name = str(with_block.get("name", ""))
    assert "${{ matrix.os }}" in name, (
        f"the artefact name {name!r} must vary by operating system, or two "
        f"lanes collide on one name"
    )
    assert "${{ strategy.job-index }}" in name, (
        f"the artefact name {name!r} must vary by matrix leg, or two legs on "
        f"one operating system collide"
    )
    assert with_block.get("retention-days") == RETENTION_DAYS, (
        f"the artefact must be kept for {RETENTION_DAYS} days, not "
        f"{with_block.get('retention-days')!r}"
    )


def test_a_missing_report_is_surfaced_rather_than_swallowed(
    build_test_job: dict[str, typ.Any],
) -> None:
    """Silence and success look identical on an absent report.

    ``if-no-files-found: ignore`` uploads nothing and says nothing, so a
    lading that stopped writing the file would read as a run whose report
    nobody happened to open. The verification step names the cause; the
    upload's own setting must not contradict it.
    """
    with_block = mapping_at(
        step_named(build_test_job, UPLOAD_STEP), "with", f"the {UPLOAD_STEP!r} step"
    )
    assert with_block.get("if-no-files-found") == "warn", (
        f"the upload must warn on a missing report, not "
        f"{with_block.get('if-no-files-found')!r}"
    )


def test_the_verification_step_reads_the_report_it_was_given(
    build_test_job: dict[str, typ.Any],
) -> None:
    """A verification that checked a different path proves nothing.

    It also must not fail the job. The report is evidence about a build,
    not the build, and a publish that failed before lading ran has
    already failed on its own account.
    """
    verify = step_named(build_test_job, VERIFY_STEP)
    environment = mapping_at(verify, "env", f"the {VERIFY_STEP!r} step")
    assert environment.get("STATS_PATH") == statistics_path(build_test_job), (
        f"{VERIFY_STEP!r} reads {environment.get('STATS_PATH')!r}, not the "
        f"{statistics_path(build_test_job)!r} the publish step writes"
    )
    script = str(verify.get("run", ""))
    assert "::warning" in script, (
        f"{VERIFY_STEP!r} must report a missing or unreadable report as a "
        f"warning; a silent check is the state this replaces"
    )
    assert "json.load" in script, (
        f"{VERIFY_STEP!r} must parse the report rather than only test that "
        f"the file exists; an empty or truncated file would pass otherwise"
    )
    assert "exit 1" not in script, (
        f"{VERIFY_STEP!r} must not fail the job: the report is evidence "
        f"about the build, not the build"
    )
