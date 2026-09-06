"""Contract tests for the lading pin and its compiler-cache reporting.

lading runs the publish dry run, and it is pinned three times: `ci.yml`
sets `LADING_REF` for the job, the Makefile sets it again so a local
`make publish-check` resolves the same tool, and `pyproject.toml` pins it
in the `python-tools` group for a bare `uv run lading`. Three pins for one
tool drift, and the failure is quiet: CI validates publish readiness with
one version while a developer validates it with another, and each
believes the other agrees.

The third is the easiest to miss, because the Makefile's `--with` overlay
masks it: `make publish-check` resolves the Makefile's pin regardless of
what the project group holds, so the group can sit generations behind
without any command failing.

The version itself is not asserted. A bump should need one paired change,
not three. What is asserted is that the two agree, and that the workflow
still asks lading for the compiler-cache statistics whose absence made
the publish step's cost unattributable (rstest-bdd#720).

Run via ``make test-workflow-contracts``.
"""

import re
import typing as typ

import pytest
from workflow_support import job, repository_file, steps

#: The workflow these assertions read, loaded through
#: :func:`workflow_support.job` so file access and YAML parsing happen at
#: one boundary rather than in each contract.
CI_WORKFLOW: typ.Final[str] = "ci.yml"

#: The job that packages the crates.
BUILD_TEST_JOB: typ.Final[str] = "build-test"

#: The step that runs the publish dry run.
DRY_RUN_STEP: typ.Final[str] = "Publish dry run"

#: The environment variable lading reads for its statistics file.
STATS_VARIABLE: typ.Final[str] = "LADING_SCCACHE_STATS_JSON"

#: The action that collects the statistics file.
UPLOAD_ACTION: typ.Final[str] = "actions/upload-artifact@"

#: The step that reads the report before it is uploaded.
VERIFY_STEP: typ.Final[str] = "Verify publish-step compiler-cache statistics"

#: The step that collects the report.
UPLOAD_STEP: typ.Final[str] = "Upload publish-step compiler-cache statistics"

#: The condition both steps carry. Written out rather than matched
#: loosely: `always()` alone would upload from the Windows lanes, which
#: never write the file, and `runner.os == 'Linux'` alone would skip the
#: run that failed, which is the run whose cost is worth reading.
LINUX_ALWAYS: typ.Final[str] = "${{ always() && runner.os == 'Linux' }}"

#: How long the artefact is kept. Asserted because an artefact that
#: expires before anyone compares two runs is not evidence.
RETENTION_DAYS: typ.Final[int] = 7

_FULL_SHA: typ.Final[re.Pattern[str]] = re.compile(r"^[0-9a-f]{40}$")


def _makefile_lading_ref() -> str:
    """Return the Makefile's lading pin.

    Returns
    -------
    str
        The commit the Makefile resolves lading from.
    """
    makefile = repository_file("Makefile")
    match = re.search(r"^LADING_REF \?= (?P<ref>\S+)$", makefile, re.MULTILINE)
    assert match is not None, "the Makefile must define LADING_REF"
    return match["ref"]


def _pyproject_lading_ref() -> str:
    """Return the project group's lading pin.

    Returns
    -------
    str
        The commit `uv` resolves lading from for a bare `uv run`.
    """
    pyproject = repository_file("pyproject.toml")
    match = re.search(
        r"lading @ git\+https://github\.com/leynos/lading@(?P<ref>[0-9a-f]{40})",
        pyproject,
    )
    assert match is not None, "pyproject.toml must pin lading by commit"
    return match["ref"]


def _build_test_steps() -> list[dict[str, typ.Any]]:
    """Return the packaging job's steps, in document order.

    Returns
    -------
    list[dict[str, typ.Any]]
        The steps of the job that runs the publish dry run.
    """
    return typ.cast("list[dict[str, typ.Any]]", steps(job(CI_WORKFLOW, BUILD_TEST_JOB)))


def _build_test_env() -> dict[str, typ.Any]:
    """Return the build-test job's environment mapping.

    Returns
    -------
    dict[str, typ.Any]
        The job-level environment.
    """
    environment = job(CI_WORKFLOW, BUILD_TEST_JOB).get("env")
    assert isinstance(environment, dict), "build-test must define env"
    return environment


def _step_index(name: str) -> int:
    """Return the position of the one step with ``name``.

    Uniqueness is part of the assertion. Two steps of the same name would
    make every ordering claim below ambiguous, and the one that mattered
    could be the one that moved.

    Parameters
    ----------
    name : str
        The step's declared name.

    Returns
    -------
    int
        Its index among the job's steps.
    """
    matches = [
        index
        for index, step in enumerate(_build_test_steps())
        if step.get("name") == name
    ]
    assert len(matches) == 1, (
        f"{BUILD_TEST_JOB} must declare exactly one {name!r} step, found {len(matches)}"
    )
    return matches[0]


def _step(name: str) -> dict[str, typ.Any]:
    """Return the one step with ``name``.

    Parameters
    ----------
    name : str
        The step's declared name.

    Returns
    -------
    dict[str, typ.Any]
        The step.
    """
    return _build_test_steps()[_step_index(name)]


def _dry_run_steps() -> list[dict[str, typ.Any]]:
    """Return every publish dry-run step in the packaging job.

    Returns
    -------
    list[dict[str, typ.Any]]
        The steps named for the dry run.
    """
    return [step for step in _build_test_steps() if step.get("name") == DRY_RUN_STEP]


def test_every_lading_pin_agrees() -> None:
    """All three places must resolve the same lading.

    Drift here is quiet rather than loud: every side keeps working, and
    each validates publish readiness against a different tool while
    believing the others agree. The project group is the easiest to
    forget, because the Makefile's `--with` overlay hides it.
    """
    pins = {
        "ci.yml": _build_test_env().get("LADING_REF"),
        "Makefile": _makefile_lading_ref(),
        "pyproject.toml": _pyproject_lading_ref(),
    }

    assert len(set(pins.values())) == 1, (
        f"every lading pin must name the same commit; a bump must move all "
        f"of them: {pins}"
    )


def test_the_lading_pin_is_a_commit() -> None:
    """A tag or branch would let the tool change under a green pin."""
    makefile_ref = _makefile_lading_ref()

    assert _FULL_SHA.match(makefile_ref), (
        f"LADING_REF must be a full 40-character commit SHA, not a tag or "
        f"branch: {makefile_ref!r}"
    )


def test_the_dry_run_asks_for_compiler_cache_statistics() -> None:
    """The publish step's cost must stay attributable.

    Without this the step is the longest in the job and says nothing
    about what it spent, which is what made the earlier 36-minute runs
    impossible to reason about.
    """
    steps = _dry_run_steps()
    assert steps, f"ci.yml must have a {DRY_RUN_STEP!r} step"

    for step in steps:
        environment = step.get("env") or {}
        assert STATS_VARIABLE in environment, (
            f"the {DRY_RUN_STEP!r} step must set {STATS_VARIABLE} so lading "
            f"reports the compiler cache around each packaged build"
        )
        assert str(environment[STATS_VARIABLE]).endswith(".json"), (
            f"{STATS_VARIABLE} must name a JSON file, got "
            f"{environment[STATS_VARIABLE]!r}"
        )


def _statistics_path() -> str:
    """Return the one path the workflow tells lading to write to.

    Returns
    -------
    str
        The configured file path.
    """
    environment = _step(DRY_RUN_STEP).get("env") or {}
    path = environment.get(STATS_VARIABLE)
    assert isinstance(path, str), (
        f"the {DRY_RUN_STEP!r} step must set {STATS_VARIABLE} to a path, got {path!r}"
    )
    assert path, f"the {DRY_RUN_STEP!r} step must set {STATS_VARIABLE}"
    return path


def test_the_statistics_file_is_uploaded() -> None:
    """A file written into RUNNER_TEMP and never uploaded is not evidence.

    The upload has to collect the path the publish step wrote, so the two
    are compared rather than each checked against a constant. A step that
    uploaded a stale path would satisfy either check alone.
    """
    upload = _step(UPLOAD_STEP)
    assert UPLOAD_ACTION in str(upload.get("uses", "")), (
        f"{UPLOAD_STEP!r} must use {UPLOAD_ACTION}, not {upload.get('uses')!r}"
    )
    with_block = upload.get("with") or {}
    assert with_block.get("path") == _statistics_path(), (
        f"{UPLOAD_STEP!r} uploads {with_block.get('path')!r}, which is not "
        f"the {_statistics_path()!r} the publish step writes; the report "
        f"would be discarded with the runner"
    )


def test_the_upload_follows_the_publish_step() -> None:
    """Ordering is the whole of it: nothing exists to upload before.

    A step order that put the upload first would pass every assertion
    about its inputs and collect nothing on every run.
    """
    publish = _step_index(DRY_RUN_STEP)
    verify = _step_index(VERIFY_STEP)
    upload = _step_index(UPLOAD_STEP)
    assert publish < verify < upload, (
        f"the publish step (index {publish}) must precede the verification "
        f"(index {verify}) and the upload (index {upload}); a report is "
        f"read and collected after it is written, not before"
    )


@pytest.mark.parametrize("step_name", [VERIFY_STEP, UPLOAD_STEP], ids=str)
def test_the_report_steps_run_on_a_failed_linux_run(step_name: str) -> None:
    """The run worth reading is often the one that failed.

    Both halves of the condition are load-bearing and each fails a
    different way. Without ``always()`` a failed publish uploads nothing,
    which is the run whose cost is worth knowing. Without the Linux
    guard, the Windows lanes, which never write the file, would report a
    missing one on every run.
    """
    condition = str(_step(step_name).get("if", ""))
    assert condition == LINUX_ALWAYS, (
        f"{step_name!r} has if: {condition!r}, not {LINUX_ALWAYS!r}"
    )


def test_the_artefact_is_named_per_lane_and_kept() -> None:
    """One name per lane, kept long enough to compare two runs.

    A name shared across the matrix collides, and the second lane's
    upload fails or overwrites the first. A retention shorter than the
    comparison window makes the artefact evidence that expires before it
    is read.
    """
    with_block = _step(UPLOAD_STEP).get("with") or {}
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


def test_a_missing_report_is_surfaced_rather_than_swallowed() -> None:
    """Silence and success look identical on an absent report.

    ``if-no-files-found: ignore`` uploads nothing and says nothing, so a
    lading that stopped writing the file would read as a run whose report
    nobody happened to open. The verification step names the cause; the
    upload's own setting must not contradict it.
    """
    with_block = _step(UPLOAD_STEP).get("with") or {}
    assert with_block.get("if-no-files-found") == "warn", (
        f"the upload must warn on a missing report, not "
        f"{with_block.get('if-no-files-found')!r}"
    )


def test_the_verification_step_reads_the_report_it_was_given() -> None:
    """A verification that checked a different path proves nothing.

    It also must not fail the job. The report is evidence about a build,
    not the build, and a publish that failed before lading ran has
    already failed on its own account.
    """
    verify = _step(VERIFY_STEP)
    environment = verify.get("env") or {}
    assert environment.get("STATS_PATH") == _statistics_path(), (
        f"{VERIFY_STEP!r} reads {environment.get('STATS_PATH')!r}, not the "
        f"{_statistics_path()!r} the publish step writes"
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
