"""Readers for the pull-request concurrency contract.

A superseded pull-request run costs the same minutes as the run that
replaced it. GitHub cancels one only when the workflow declares a
concurrency group and asks for it, so the fact is a property of every
workflow a pull request can start, not of any one job.

The readers here are pure: :func:`is_pull_request_startable` and
:func:`concurrency_violations` take a parsed document, so the contract
can drive them with a synthetic workflow that the repository does not
contain. A rule exercised only over files that already satisfy it
passes whether or not it works. Triggers are read by
:func:`pull_request_reach.trigger_names`, the reader the CodeScene
closure uses, rather than by a second copy of it.

Shape faults raise :class:`workflow_support.WorkflowShapeError`
subclasses rather than asserting, matching the rest of the harness.
"""

import typing as typ

from pull_request_reach import trigger_names
from workflow_support import workflow

#: The trigger that a pull request starts. ``pull_request_target`` runs
#: with the base repository's token and is deliberately excluded: those
#: workflows here push commits and merge, and cancelling one mid-write
#: is not a saving.
PULL_REQUEST_TRIGGER: typ.Final = "pull_request"

#: The only accepted ``cancel-in-progress`` value. A literal ``true``
#: would cancel a push to main or a scheduled run that shares the
#: group, so the contract requires the guarded expression and not
#: merely a truthy setting.
CANCEL_IN_PROGRESS: typ.Final = "${{ github.event_name == 'pull_request' }}"

#: Values that change from one run of the same pull request to the next. A
#: group keyed on any of them is unique per run, so it serializes nothing
#: and can never cancel the run it supersedes.
PER_RUN_EXPRESSIONS: typ.Final = (
    "github.run_id",
    "github.run_number",
    "github.run_attempt",
    "github.sha",
)
#: Values that stay the same across pushes to one pull request and differ
#: between pull requests. A group must carry one: without it, a static
#: group such as "ci" makes every pull request cancel every other.
STABLE_DISCRIMINATORS: typ.Final = (
    "github.ref",
    "github.head_ref",
    "github.event.pull_request.number",
)


def is_pull_request_startable(document: dict[str, object]) -> bool:
    """Report whether a pull request can start this workflow.

    Parameters
    ----------
    document : dict[str, object]
        A parsed workflow document.

    Returns
    -------
    bool
        True when the document declares the ``pull_request`` trigger. A
        trigger block that is absent, ambiguous or of no accepted form is
        refused by :func:`pull_request_reach.trigger_names` with
        :class:`pull_request_reach.TriggerShapeError`.
    """
    return PULL_REQUEST_TRIGGER in trigger_names(document)


def concurrency_violations(document: dict[str, object]) -> list[str]:
    """Return every way a document fails the concurrency contract.

    Parameters
    ----------
    document : dict[str, object]
        A parsed workflow document.

    Returns
    -------
    list[str]
        One message per violation; empty when the document conforms.
    """
    concurrency = document.get("concurrency")
    if concurrency is None:
        return ["declares no concurrency: block"]
    if not isinstance(concurrency, dict):
        return ["declares a concurrency: that is not a mapping"]
    return _group_violations(concurrency.get("group")) + _cancel_violations(
        concurrency.get("cancel-in-progress")
    )


def _group_violations(group: object) -> list[str]:
    """Return the violations of the concurrency group itself.

    The group must separate pull requests from one another and hold one pull
    request's runs together: a stable discriminator such as ``github.ref``,
    and no value that changes per run.

    Parameters
    ----------
    group : object
        The declared ``group`` value.

    Returns
    -------
    list[str]
        One message per violation; empty when the group conforms.
    """
    if not isinstance(group, str) or not group.strip():
        return ["declares no concurrency group"]
    per_run = [value for value in PER_RUN_EXPRESSIONS if value in group]
    if per_run:
        return [f"keys its concurrency group on {', '.join(per_run)}"]
    if not any(value in group for value in STABLE_DISCRIMINATORS):
        return [
            f"keys its concurrency group on none of {', '.join(STABLE_DISCRIMINATORS)}"
        ]
    return []


def _cancel_violations(cancel: object) -> list[str]:
    """Return the violations of the ``cancel-in-progress`` setting.

    Parameters
    ----------
    cancel : object
        The declared ``cancel-in-progress`` value.

    Returns
    -------
    list[str]
        One message per violation; empty when the setting conforms.
    """
    if cancel is None:
        return ["sets no cancel-in-progress"]
    if not isinstance(cancel, str) or " ".join(cancel.split()) != CANCEL_IN_PROGRESS:
        return [f"sets cancel-in-progress to {cancel!r} and not {CANCEL_IN_PROGRESS}"]
    return []


def pull_request_workflows(names: list[str]) -> dict[str, dict[str, object]]:
    """Load the workflows a pull request can start.

    Parameters
    ----------
    names : list[str]
        Workflow file names under ``.github/workflows``.

    Returns
    -------
    dict[str, dict[str, object]]
        Each pull-request-startable workflow, keyed by file name.
    """
    documents = {name: workflow(name) for name in names}
    return {
        name: document
        for name, document in documents.items()
        if is_pull_request_startable(document)
    }
