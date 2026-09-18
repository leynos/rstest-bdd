"""Reading a workflow's CodeScene interactions, and the estate's shape rules.

Split from :mod:`codescene_coverage_test` so the scanner can be driven over
documents written for one interaction each. A rule parametrized over
`.github/workflows` alone proves only that those files pass today, which they
do whether or not the scanner recognizes anything.

See Also
--------
codescene_coverage_test : The CV-005 contracts built on these.
"""

import re
import typing as typ

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

#: Events that let a pull request's head decide what runs.
PULL_REQUEST_EVENTS = frozenset({"pull_request", "pull_request_target"})

SHA_PINNED = re.compile(
    r"^leynos/shared-actions/\.github/actions/"
    r"(generate-coverage|upload-codescene-coverage)@[0-9a-f]{40}$"
)

#: What a CodeScene interaction looks like, wherever it is written. The
#: token is named separately from the action because a lane can be given the
#: credential without calling the action, and a credential a pull request's
#: head can reach is the thing CV-005 is really about.
MARKERS = {
    "a CodeScene action": "codescene",
    "a cs-coverage command": "cs-coverage",
    "the CodeScene credential": "CS_ACCESS_TOKEN",
}


class MissingTriggerBlockError(WorkflowShapeError):
    """A workflow declares no trigger mapping.

    Parameters
    ----------
    workflow_name : str
        The workflow that declares none.
    declared : object
        What was found instead.
    """

    def __init__(self, workflow_name: str, declared: object) -> None:
        super().__init__(
            f"{workflow_name} must declare an on: mapping; got {declared!r}"
        )


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
    declared at workflow scope, at job scope, on a step, or passed as an action
    input, and a scan that read only ``uses`` and ``run`` would miss three of
    those four.

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
    return sorted(set(found))


def codescene_references(workflow_name: str) -> list[str]:
    """Return every place a workflow interacts with CodeScene.

    Parameters
    ----------
    workflow_name : str
        The workflow file name.

    Returns
    -------
    list[str]
        One entry per interaction, naming what was found and where.
    """
    return references_in(workflow(workflow_name), workflow_name)


def triggers(workflow_name: str) -> dict[str, object]:
    """Return a workflow's trigger block.

    Parameters
    ----------
    workflow_name : str
        The workflow file name.

    Returns
    -------
    dict[str, object]
        The declared events. PyYAML reads the bare word ``on`` as the
        boolean ``True``, so both keys are tried.

    Raises
    ------
    MissingTriggerBlockError
        If the workflow declares no trigger mapping.
    """
    document = workflow(workflow_name)
    declared = document.get("on", document.get(True))
    if not isinstance(declared, dict):
        raise MissingTriggerBlockError(workflow_name, declared)
    return declared


def pull_request_workflows() -> list[str]:
    """Return every workflow a pull request's head can reach.

    Returns
    -------
    list[str]
        Sorted workflow file names.
    """
    return sorted(
        name for name in workflow_names() if PULL_REQUEST_EVENTS & set(triggers(name))
    )


def coverage_step(workflow_name: str, step_name: str) -> dict[str, object]:
    """Return one coverage step's inputs, named by its location if absent.

    Parameters
    ----------
    workflow_name : str
        The workflow file name.
    step_name : str
        The step's declared name.

    Returns
    -------
    dict[str, object]
        The step's ``with`` mapping.

    Raises
    ------
    MissingCoverageStepError
        If the workflow declares no step of that name.
    MissingStepInputsError
        If the step declares no inputs.
    """
    for reference in iter_steps(workflow_name):
        if reference.name != step_name:
            continue
        inputs = reference.step.get("with")
        if not isinstance(inputs, dict):
            raise MissingStepInputsError(str(reference))
        return inputs
    found = [reference.name for reference in iter_steps(workflow_name)]
    raise MissingCoverageStepError(workflow_name, step_name, found)


#: One document per marker, each carrying exactly the interaction its marker
#: exists to find, and each written where a real workflow would put it: the
#: action on a step, the command in a shell script, the credential in job
#: scope. A marker is proved against its own document, not against the
#: repository's files, which pass a broken scanner just as readily.
MARKER_FIXTURES = {
    "a CodeScene action": {
        "jobs": {
            "gate": {
                "steps": [
                    {
                        "uses": (
                            "leynos/shared-actions/.github/actions/"
                            "upload-codescene-coverage@" + "0" * 40
                        )
                    }
                ]
            }
        }
    },
    "a cs-coverage command": {
        "jobs": {
            "gate": {"steps": [{"run": "cs-coverage check --coverage-files c.xml"}]}
        }
    },
    "the CodeScene credential": {
        "jobs": {
            "gate": {
                "env": {"CS_ACCESS_TOKEN": "${{ secrets.CS_ACCESS_TOKEN }}"},
                "steps": [{"run": "true"}],
            }
        }
    },
}


#: Inputs that select what is measured. Publication differs between the two
#: lanes by design and is asserted separately.
SELECTION_INPUTS = (
    "cache-provider",
    "output-path",
    "format",
    "all-features",
    "all-targets",
    "doctests",
    "use-cargo-nextest",
    "pytest-workers",
    "with-ratchet",
)
