"""Which workflows a pull request's head can run.

A pull-request lane is a closure, not a trigger list. A workflow that declares
only ``workflow_call`` still runs on a pull request when a pull-request
workflow calls it, and ``secrets: inherit`` hands it every secret the caller
holds. episodic #341 measured the hole this closes: a ``workflow_call``-only
workflow, called from a pull-request job with ``secrets: inherit`` and curling
the CodeScene API with the inherited token, passed thirteen contract tests
that enumerated triggers alone.

Every function here is pure over parsed documents, so each reading can be
driven over documents written for it. This repository calls no local reusable
workflow today, so read from its own files a traversal that followed nothing
would pass; the exposure is what a later change would introduce.

See Also
--------
codescene_coverage_support.pull_request_workflows : The closure over this
    repository's workflows.
"""

import posixpath
import typing as typ

from workflow_support import WorkflowShapeError

if typ.TYPE_CHECKING:
    import collections.abc as cabc

#: Events that let a pull request's head decide what runs.
PULL_REQUEST_EVENTS: typ.Final = frozenset({"pull_request", "pull_request_target"})
#: Where GitHub reads a repository's workflows. A same-repository call is any
#: reference whose path resolves here, whatever spelling reached it.
WORKFLOW_DIRECTORY: typ.Final[str] = ".github/workflows"
#: This repository, as a remote reference to one of its own workflows names it.
REPOSITORY: typ.Final[str] = "leynos/rstest-bdd"
#: An `owner/repo/path` reference has at least two separators before the path.
_REMOTE_SEPARATORS: typ.Final[int] = 2


class TriggerShapeError(WorkflowShapeError):
    """A workflow's trigger block is absent, ambiguous, or of no known form.

    Parameters
    ----------
    subject : str
        The workflow named in the message.
    detail : str
        What was wrong.
    """

    def __init__(self, subject: str, detail: str) -> None:
        super().__init__(f"{subject}: {detail}")


class UnrecognizedCallError(WorkflowShapeError):
    """A job-level ``uses`` is neither a local call nor a remote reference.

    Refused rather than skipped: a spelling this reader does not know is
    exactly the one that would drop a caller from the closure in silence.

    Parameters
    ----------
    reference : str
        The job's ``uses`` value.
    """

    def __init__(self, reference: str) -> None:
        super().__init__(
            f"{reference!r} is neither a path under {WORKFLOW_DIRECTORY} nor an "
            "owner/repo/path@ref reference; refusing to guess what it calls"
        )


class MissingCalledWorkflowError(WorkflowShapeError):
    """A local call names a workflow file that does not exist.

    Parameters
    ----------
    reference : str
        The job's ``uses`` value.
    """

    def __init__(self, reference: str) -> None:
        super().__init__(
            f"{reference!r} calls a workflow that is not in {WORKFLOW_DIRECTORY}; "
            "the closure cannot be computed over a file it cannot read"
        )


def trigger_names(document: object, subject: str = "workflow") -> frozenset[str]:
    """Return the event names a parsed workflow declares, in any form.

    GitHub accepts ``on: push``, ``on: [push, pull_request]`` and the mapping
    form. A reader expecting only the mapping stringifies the list into one
    key named after the whole list, and that workflow then escapes every
    pull-request rule. YAML 1.1 also reads the bare word ``on`` as the boolean
    ``True``, so both keys are read.

    Parameters
    ----------
    document : object
        A parsed workflow.
    subject : str
        What to name in an error.

    Returns
    -------
    frozenset[str]
        The declared event names.

    Raises
    ------
    TriggerShapeError
        If the document declares no trigger block, declares it under both
        keys, or declares it in no form GitHub accepts.

    Examples
    --------
    >>> sorted(trigger_names({True: ["push", "pull_request"]}))
    ['pull_request', 'push']
    """
    if not isinstance(document, dict):
        raise TriggerShapeError(subject, "must parse to a mapping")
    present = [key for key in ("on", True) if key in document]
    if len(present) != 1:
        raise TriggerShapeError(
            subject, f"must declare exactly one trigger block; found {present}"
        )
    return _event_names(document[present[0]], subject)


def _event_names(declared: object, subject: str) -> frozenset[str]:
    """Read the three trigger forms GitHub accepts.

    Parameters
    ----------
    declared : object
        The value under the trigger key.
    subject : str
        What to name in an error.

    Returns
    -------
    frozenset[str]
        The declared event names.

    Raises
    ------
    TriggerShapeError
        If the value is none of a string, a list of strings, or a mapping.
    """
    match declared:
        case str():
            return frozenset({declared})
        case list() if declared and all(isinstance(entry, str) for entry in declared):
            return frozenset(declared)
        case dict() if declared:
            return frozenset(str(key) for key in declared)
        case _:
            raise TriggerShapeError(
                subject, f"declares triggers in no form GitHub accepts: {declared!r}"
            )


def _local_path(reference: str) -> str | None:
    """Return the repository path a same-repository call names, if it is one.

    Matched by shape rather than by an enumerated prefix list. GitHub's
    recommended ``$/`` self-repository prefix is stripped when no ``@ref``
    follows it, since a ``$/`` reference may not carry one; this repository's
    own ``owner/repo/`` and any ``@ref`` are stripped otherwise; and
    normalizing the path removes a leading ``./``. What remains must be a
    file directly under the workflow directory.

    Parameters
    ----------
    reference : str
        A job-level ``uses`` value.

    Returns
    -------
    str | None
        The file name under the workflow directory, or ``None`` when the
        reference is not a same-repository call.
    """
    path, separator, _ref = reference.partition("@")
    if path.startswith("$/"):
        return None if separator else _in_workflow_directory(path.removeprefix("$/"))
    return _in_workflow_directory(path.removeprefix(f"{REPOSITORY}/"))


def _in_workflow_directory(path: str) -> str | None:
    """Return the file name when a path resolves directly under the directory.

    Parameters
    ----------
    path : str
        A repository-relative path.

    Returns
    -------
    str | None
        The file name, or ``None`` when the path is anywhere else.
    """
    directory, name = posixpath.split(posixpath.normpath(path))
    return name if directory == WORKFLOW_DIRECTORY and name else None


def _is_remote_reference(reference: str) -> bool:
    """Report whether a ``uses`` value is an ``owner/repo/path@ref`` call.

    Parameters
    ----------
    reference : str
        A job-level ``uses`` value.

    Returns
    -------
    bool
        True for a reference into another repository.
    """
    path, separator, ref = reference.partition("@")
    return (
        bool(separator and ref)
        and not path.startswith("$/")
        and path.count("/") >= _REMOTE_SEPARATORS
        and ".." not in path
    )


def calls_in(document: object, present: cabc.Collection[str]) -> frozenset[str]:
    """Return the same-repository workflows a parsed document calls.

    Only a job-level ``uses`` is a workflow call; a step-level ``uses`` is an
    action. A remote reusable workflow is not followed: its text is not here
    to read, and the credential reaches it only through ``secrets:``, which
    the CodeScene scan reads in the caller.

    Parameters
    ----------
    document : object
        A parsed workflow.
    present : cabc.Collection[str]
        File names that exist in the workflow directory.

    Returns
    -------
    frozenset[str]
        File names this document calls.

    A job-level ``uses`` of neither known shape fails with
    :class:`UnrecognizedCallError`, and a local call naming a file that does
    not exist fails with :class:`MissingCalledWorkflowError`, both raised by
    :func:`_called_workflow`.
    """
    declared = document.get("jobs") if isinstance(document, dict) else None
    jobs = declared.values() if isinstance(declared, dict) else ()
    called = (
        _called_workflow(str(job["uses"]), present)
        for job in jobs
        if isinstance(job, dict) and job.get("uses") is not None
    )
    return frozenset(name for name in called if name is not None)


def _called_workflow(reference: str, present: cabc.Collection[str]) -> str | None:
    """Return the local workflow one job-level ``uses`` calls, if any.

    Parameters
    ----------
    reference : str
        The job's ``uses`` value.
    present : cabc.Collection[str]
        File names that exist in the workflow directory.

    Returns
    -------
    str | None
        The called file name, or ``None`` for a remote reusable workflow.

    Raises
    ------
    UnrecognizedCallError
        If the reference has neither known shape.
    MissingCalledWorkflowError
        If a local call names a file that does not exist.
    """
    name = _local_path(reference)
    if name is None and not _is_remote_reference(reference):
        raise UnrecognizedCallError(reference)
    if name is not None and name not in present:
        raise MissingCalledWorkflowError(reference)
    return name


def pull_request_closure(documents: cabc.Mapping[str, object]) -> frozenset[str]:
    """Return every workflow a pull request reaches, following local calls.

    Parameters
    ----------
    documents : cabc.Mapping[str, object]
        File name to parsed document, for every workflow in the directory.

    Returns
    -------
    frozenset[str]
        The workflows a pull-request event triggers, and everything they call,
        transitively.

    Examples
    --------
    >>> sorted(pull_request_closure({
    ...     "ci.yml": {True: "pull_request",
    ...                "jobs": {"a": {"uses": "./.github/workflows/lib.yml"}}},
    ...     "lib.yml": {True: "workflow_call", "jobs": {}},
    ... }))
    ['ci.yml', 'lib.yml']
    """
    pending = [
        name
        for name, document in documents.items()
        if PULL_REQUEST_EVENTS & trigger_names(document, name)
    ]
    reached: set[str] = set()
    while pending:
        current = pending.pop()
        if current in reached:
            continue
        reached.add(current)
        pending.extend(calls_in(documents[current], documents) - reached)
    return frozenset(reached)
