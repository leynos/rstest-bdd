"""Where the coverage lanes sit, and the readers the remaining contracts share.

The workflow and step names the lane and publisher contracts address, the
reading of a coverage step's inputs, and the pull-request closure with the
scan for CodeScene references that `pull_request_reach_test` drives. The CV-005
rules themselves are held by `cv005-contracts check`.
"""

import typing as typ

from pull_request_reach import pull_request_closure
from workflow_queries import iter_steps, workflow_names
from workflow_support import WorkflowShapeError, workflow

if typ.TYPE_CHECKING:
    import collections.abc as cabc

#: The workflow that owns the trunk generation and the upload.
PUBLISHER = "coverage-main.yml"
#: The pull-request lane that measures for the ratchet.
PR_WORKFLOW = "ci.yml"
PR_COVERAGE_STEP = "Test and Measure Coverage (Linux)"
PUBLISHER_COVERAGE_STEP = "Test and Measure Coverage"
#: The publisher job that uploads. A second job writes the Windows baseline
#: and uploads nothing.
PUBLISHER_JOB = "coverage-upload"


#: What a CodeScene interaction looks like, wherever it is written. The
#: token is named separately from the action because a lane can be given the
#: credential without calling the action, and a credential a pull request's
#: head can reach is the thing CV-005 is really about.
MARKERS = {
    "a CodeScene action": "codescene",
    "a cs-coverage command": "cs-coverage",
    "the CodeScene credential": "CS_ACCESS_TOKEN",
}
#: A job forwarding every secret its caller holds. It names no credential, so
#: no text marker finds it, and it is how the token reaches a reusable
#: workflow, remote or local, that the caller's own text never mentions.
INHERITED_SECRETS = "every secret forwarded by secrets: inherit"


class MissingCoverageStepError(WorkflowShapeError):
    """A workflow does not declare the coverage step a contract names.

    Parameters
    ----------
    workflow_name : str
        The workflow searched.
    step_name : str
        The step looked for.
    found : list[str]
        The step names the workflow does declare.
    """

    def __init__(self, workflow_name: str, step_name: str, found: list[str]) -> None:
        super().__init__(
            f"{workflow_name} must declare a {step_name!r} step; it has {found}"
        )


class MissingStepInputsError(WorkflowShapeError):
    """A step that must configure an action declares no inputs.

    Parameters
    ----------
    where : str
        The step's location.
    """

    def __init__(self, where: str) -> None:
        super().__init__(f"{where} must declare a with: mapping")


def _walk(value: object, path: str) -> cabc.Iterator[tuple[str, str]]:
    """Yield every scalar in a parsed document with the path that reached it.

    Parameters
    ----------
    value : object
        A parsed YAML fragment.
    path : str
        The path taken to reach it.

    Yields
    ------
    tuple[str, str]
        The path and the scalar rendered as text.
    """
    match value:
        case dict():
            for key, entry in value.items():
                yield f"{path}.{key}", str(key)
                yield from _walk(entry, f"{path}.{key}")
        case list():
            for position, entry in enumerate(value):
                yield from _walk(entry, f"{path}[{position}]")
        case _:
            yield path, str(value)


def references_in(document: object, subject: str) -> list[str]:
    """Return every place a parsed document interacts with CodeScene.

    The whole document is walked, not a list of step keys. A credential can be
    declared at workflow scope, at job scope, on a step, passed as an action
    input, interpolated into a ``run`` body, or forwarded to a reusable
    workflow by name, and a scan that read only ``uses`` and ``run`` would
    miss most of those. Forwarding by ``secrets: inherit`` names nothing, so
    it is recognized by its position instead.

    Pure, so the markers can be driven over a document written to carry one
    interaction each. Driven over this repository's workflows alone, a marker
    that had stopped matching would report a clean estate.

    Parameters
    ----------
    document : object
        A parsed workflow.
    subject : str
        What to name in the reported location.

    Returns
    -------
    list[str]
        One entry per interaction, naming what was found and where.
    """
    found = []
    for path, text in _walk(document, subject):
        lowered = text.lower()
        found.extend(
            f"{description} at {path}"
            for description, marker in MARKERS.items()
            if marker.lower() in lowered
        )
        if path.endswith(".secrets") and lowered.strip() == "inherit":
            found.append(f"{INHERITED_SECRETS} at {path}")
    return sorted(set(found))


def pull_request_workflows(
    documents: cabc.Mapping[str, object] | None = None,
) -> list[str]:
    """Return every workflow a pull request's head can reach.

    The closure through same-repository calls, not the trigger list: a
    ``workflow_call``-only workflow a pull-request job calls runs on that pull
    request and, under ``secrets: inherit``, holds every secret its caller
    does.

    Parameters
    ----------
    documents : cabc.Mapping[str, object] | None
        File name to parsed document. ``None`` reads this repository's
        workflows; a contract passes its own to drive the composition over a
        call chain this repository does not contain.

    Returns
    -------
    list[str]
        Sorted workflow file names.
    """
    if documents is None:
        documents = {name: workflow(name) for name in workflow_names()}
    return sorted(pull_request_closure(documents))


def coverage_step(
    workflow_name: str, step_name: str, job_name: str
) -> dict[str, object]:
    """Return one coverage step's inputs, named by its location if absent.

    The job is named, not inferred from the first match: the publisher
    declares a step of the same name in each of its jobs, so a lookup by step
    name alone would select whichever job happens to come first and follow a
    reordering silently.

    Parameters
    ----------
    workflow_name : str
        The workflow file name.
    step_name : str
        The step's declared name.
    job_name : str
        The job that declares it.

    Returns
    -------
    dict[str, object]
        The step's ``with`` mapping.

    Raises
    ------
    MissingCoverageStepError
        If the job declares no step of that name.
    MissingStepInputsError
        If the step declares no inputs.
    """
    for reference in iter_steps(workflow_name):
        if reference.job != job_name or reference.name != step_name:
            continue
        inputs = reference.step.get("with")
        if not isinstance(inputs, dict):
            raise MissingStepInputsError(str(reference))
        return inputs
    found = [
        reference.name
        for reference in iter_steps(workflow_name)
        if reference.job == job_name
    ]
    raise MissingCoverageStepError(f"{workflow_name}:{job_name}", step_name, found)
