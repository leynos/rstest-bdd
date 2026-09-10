"""Shared vocabulary for the publish-report workflow contracts.

Three contract modules read the same `build-test` job: the lading pins,
the shape of the report's verification and upload, and the verification
script run for real. The job, the step lookup and the mapping guard live
here so the three ask the same questions of the same parse rather than
each growing its own.

Run via ``make test-workflow-contracts``.
"""

import os
import re
import shutil
import subprocess  # ruff: ignore[suspicious-subprocess-import] - runs a script this repository declares.
import typing as typ

if typ.TYPE_CHECKING:
    from pathlib import Path

from workflow_support import WorkflowShapeError, job, steps

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

#: The shell the verification step declares, resolved to an absolute
#: path so the harness starts the same interpreter the runner does
#: rather than whichever `bash` a contributor's PATH offers first.
BASH: typ.Final[str] = shutil.which("bash") or "/bin/bash"

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

FULL_SHA: typ.Final[re.Pattern[str]] = re.compile(r"^[0-9a-f]{40}$")


class PublishReportShapeError(WorkflowShapeError):
    """The workflow or a pin file did not have the shape these read.

    Raised rather than asserted, because this module is not a test
    module and its assertions would be stripped under ``-O``. It derives
    from :class:`workflow_support.WorkflowShapeError`, so a shape
    violation reads as a failed expectation in test output rather than
    as an unexpected crash, exactly as the workflow loaders' do.

    Parameters
    ----------
    message : str
        What was expected, and what was found instead.
    """

    def __init__(self, message: str) -> None:
        super().__init__(message)


def _require(condition: object, message: str) -> None:
    """Raise :class:`PublishReportShapeError` when a condition fails.

    Parameters
    ----------
    condition : object
        The condition to check for truthiness.
    message : str
        What to report when it does not hold.

    Raises
    ------
    PublishReportShapeError
        If the condition is falsy.
    """
    if not condition:
        raise PublishReportShapeError(message)


def build_test_job_document() -> dict[str, typ.Any]:
    """Return the packaging job, parsed from the workflow.

    Returns
    -------
    dict[str, typ.Any]
        The job that runs the publish dry run.
    """
    return typ.cast("dict[str, typ.Any]", job(CI_WORKFLOW, BUILD_TEST_JOB))


def build_test_steps(build_test_job: dict[str, typ.Any]) -> list[dict[str, typ.Any]]:
    """Return the packaging job's steps, in document order.

    Parameters
    ----------
    build_test_job : dict[str, typ.Any]
        The parsed job.

    Returns
    -------
    list[dict[str, typ.Any]]
        The steps of the job that runs the publish dry run.
    """
    return typ.cast("list[dict[str, typ.Any]]", steps(build_test_job))


def mapping_at(owner: object, key: str, subject: str) -> dict[str, typ.Any]:
    """Return a nested mapping, failing with the location when it is not.

    ``(step.get("env") or {})`` accepts a non-empty scalar or list and
    then raises ``AttributeError`` on the next ``.get``, which reports a
    Python fault rather than the workflow shape that caused it. This
    reports the shape.

    Parameters
    ----------
    owner : object
        The step or job the mapping belongs to.
    key : str
        The key holding the mapping, such as ``"env"`` or ``"with"``.
    subject : str
        What to name in the failure message.

    Returns
    -------
    dict[str, typ.Any]
        The mapping, or an empty one when the key is absent.

    Raises
    ------
    PublishReportShapeError
        If the owner is not a mapping, or the key holds something that
        is neither a mapping nor absent.
    """
    match owner:
        case dict():
            pass
        case _:
            message = f"{subject} must be a mapping, got {owner!r}"
            raise PublishReportShapeError(message)
    match owner.get(key):
        case None:
            return {}
        case dict() as value:
            return value
        case value:
            message = f"{subject}'s {key!r} must be a mapping, got {value!r}"
            raise PublishReportShapeError(message)


def build_test_env(build_test_job: dict[str, typ.Any]) -> dict[str, typ.Any]:
    """Return the build-test job's environment mapping.

    Parameters
    ----------
    build_test_job : dict[str, typ.Any]
        The parsed job.

    Returns
    -------
    dict[str, typ.Any]
        The job-level environment.
    """
    environment = mapping_at(build_test_job, "env", f"the {BUILD_TEST_JOB} job")
    _require(environment, f"{BUILD_TEST_JOB} must define env")
    return environment


def step_index(build_test_job: dict[str, typ.Any], name: str) -> int:
    """Return the position of the one step with ``name``.

    Uniqueness is part of the assertion. Two steps of the same name would
    make every ordering claim below ambiguous, and the one that mattered
    could be the one that moved.

    Parameters
    ----------
    build_test_job : dict[str, typ.Any]
        The parsed job.
    name : str
        The step's declared name.

    Returns
    -------
    int
        Its index among the job's steps.
    """
    matches = [
        index
        for index, step in enumerate(build_test_steps(build_test_job))
        if step.get("name") == name
    ]
    _require(
        len(matches) == 1,
        f"{BUILD_TEST_JOB} must declare exactly one {name!r} step, found "
        f"{len(matches)}",
    )
    return matches[0]


def step_named(build_test_job: dict[str, typ.Any], name: str) -> dict[str, typ.Any]:
    """Return the one step with ``name``.

    Parameters
    ----------
    build_test_job : dict[str, typ.Any]
        The parsed job.
    name : str
        The step's declared name.

    Returns
    -------
    dict[str, typ.Any]
        The step.
    """
    steps_ = build_test_steps(build_test_job)
    return steps_[step_index(build_test_job, name)]


def dry_run_steps(build_test_job: dict[str, typ.Any]) -> list[dict[str, typ.Any]]:
    """Return every publish dry-run step in the packaging job.

    Parameters
    ----------
    build_test_job : dict[str, typ.Any]
        The parsed job.

    Returns
    -------
    list[dict[str, typ.Any]]
        The steps named for the dry run.
    """
    return [
        step
        for step in build_test_steps(build_test_job)
        if step.get("name") == DRY_RUN_STEP
    ]


def statistics_path(build_test_job: dict[str, typ.Any]) -> str:
    """Return the one path the workflow tells lading to write to.

    Parameters
    ----------
    build_test_job : dict[str, typ.Any]
        The parsed job.

    Returns
    -------
    str
        The configured file path.

    Raises
    ------
    PublishReportShapeError
        If the step sets no report path, or sets one that is not a
        string.
    """
    step = step_named(build_test_job, DRY_RUN_STEP)
    environment = mapping_at(step, "env", f"the {DRY_RUN_STEP!r} step")
    match environment.get(STATS_VARIABLE):
        case str() as path:
            pass
        case path:
            message = (
                f"the {DRY_RUN_STEP!r} step must set {STATS_VARIABLE} to a "
                f"path, got {path!r}"
            )
            raise PublishReportShapeError(message)
    _require(path, f"the {DRY_RUN_STEP!r} step must set {STATS_VARIABLE}")
    return path


def verification_script(build_test_job: dict[str, typ.Any]) -> str:
    """Return the verification step's shell script.

    Parameters
    ----------
    build_test_job : dict[str, typ.Any]
        The parsed job.

    Returns
    -------
    str
        The script the runner executes.

    Raises
    ------
    PublishReportShapeError
        If the verification step declares no run script, or an empty
        one.
    """
    match step_named(build_test_job, VERIFY_STEP).get("run"):
        case str() as script:
            pass
        case script:
            message = f"{VERIFY_STEP!r} must declare a run script, got {script!r}"
            raise PublishReportShapeError(message)
    _require(script.strip(), f"{VERIFY_STEP!r}'s run script is empty")
    return script


def run_verification(
    build_test_job: dict[str, typ.Any], tmp_path: Path, contents: bytes | None
) -> subprocess.CompletedProcess[str]:
    """Run the workflow's verification script against one report.

    The script is written to a file and run as ``bash <file>`` rather
    than passed to ``bash -c``. Bash 3.2 exec-replaces itself with the
    last external command of a ``-c`` string, so a harness using that
    form measures something the runner never does; runners execute
    fragments from a file.

    The report is deliberately placed outside the working directory
    the script runs in. The runner resolves the report through
    ``$STATS_PATH`` from the step's environment, not through the
    working directory, so a script that read the bare file name would
    pass a harness that put both in one directory and fail on the
    runner, where no such file exists next to the fragment. The
    harness mirrors that geometry so the contract catches the mistake
    before CI does.

    Parameters
    ----------
    build_test_job : dict[str, typ.Any]
        The parsed job.
    tmp_path : Path
        A directory to hold the script's working directory and the
        report's, kept separate so the report is not reachable from the
        script's working directory.
    contents : bytes or None
        What to write to the report path, or None to leave it absent.

    Returns
    -------
    subprocess.CompletedProcess[str]
        The finished process, with output captured.
    """
    working_dir = tmp_path / "workdir"
    report_dir = tmp_path / "report"
    working_dir.mkdir()
    report_dir.mkdir()

    script_path = working_dir / "verify.sh"
    script_path.write_text(verification_script(build_test_job), encoding="utf-8")
    stats_path = report_dir / "sccache-publish.json"
    if contents is not None:
        stats_path.write_bytes(contents)
    return subprocess.run(  # ruff: ignore[subprocess-without-shell-equals-true] - the script is this repository's own.
        [BASH, str(script_path)],
        check=False,
        capture_output=True,
        text=True,
        env={**os.environ, "STATS_PATH": str(stats_path)},
        cwd=working_dir,
    )
