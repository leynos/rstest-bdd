"""Shared vocabulary for the publish-report workflow contracts.

Three contract modules read the same `build-test` job: the lading pins,
the shape of the report's verification and upload, and the verification
script run for real. The job, the step lookup and the mapping guard live
here so the three ask the same questions of the same parse rather than
each growing its own.

Run via ``make test-workflow-contracts``.
"""

import re
import typing as typ

from workflow_support import (
    WorkflowShapeError,
    job_from_document,
    parse_workflow,
    repository_file,
    steps,
)

#: The workflow these assertions read. Its text is fetched through
#: :func:`workflow_support.repository_file` and parsed by
#: :func:`workflow_support.parse_workflow`, so repository access and
#: YAML parsing each happen at one boundary rather than in each contract.
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


def build_test_job_from_document(
    workflow_document: dict[str, typ.Any],
) -> dict[str, typ.Any]:
    """Extract the packaging job from an already-parsed workflow.

    A pure function: no repository access. The repository-backed form
    is :func:`read_build_test_job_document`.

    Parameters
    ----------
    workflow_document : dict[str, typ.Any]
        The parsed workflow mapping, as returned by
        :func:`workflow_support.parse_workflow`.

    Returns
    -------
    dict[str, typ.Any]
        The job that runs the publish dry run.

    """
    return typ.cast(
        "dict[str, typ.Any]",
        job_from_document(workflow_document, BUILD_TEST_JOB),
    )


def read_build_test_job_document() -> dict[str, typ.Any]:
    """Read the repository workflow and return its packaging job.

    A thin wrapper over the repository boundary: it reads
    :data:`CI_WORKFLOW` through :func:`workflow_support.repository_file`
    and delegates to the pure :func:`build_test_job_from_document`.
    Exists only for contract fixtures that must start from the
    repository's own content.

    Returns
    -------
    dict[str, typ.Any]
        The job that runs the publish dry run.
    """
    workflow_text = repository_file(".github", "workflows", CI_WORKFLOW)
    workflow_document = parse_workflow(workflow_text)
    return build_test_job_from_document(workflow_document)


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
