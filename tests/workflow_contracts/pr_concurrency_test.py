"""Every pull-request-startable workflow cancels superseded runs.

Pushing twice to a pull request in quick succession leaves the first
run charging minutes for a result nobody will read. GitHub cancels it
only when the workflow declares a concurrency group keyed on the pull
request and sets ``cancel-in-progress``.

The sweep below reads the repository's own workflows. The cases after
it drive the readers directly with synthetic documents, because a rule
parametrized over files that already conform passes whether or not it
discriminates: the synthetic cases are what prove it rejects a missing
line, a literal ``true`` that would cancel a push to main, and a group
keyed on the run identifier that serializes nothing.

Run via ``make test-workflow-contracts``.
"""

import typing as typ

import pytest
from pr_concurrency_support import (
    CANCEL_IN_PROGRESS,
    concurrency_violations,
    is_pull_request_startable,
    pull_request_workflows,
)
from pull_request_reach import TriggerShapeError
from workflow_queries import workflow_names

PULL_REQUEST_WORKFLOWS: typ.Final = pull_request_workflows(workflow_names())


def _document(**concurrency: object) -> dict[str, object]:
    """Build a minimal pull-request workflow with the given concurrency.

    Parameters
    ----------
    **concurrency : object
        The keys of the workflow's ``concurrency`` block.

    Returns
    -------
    dict[str, object]
        A parsed-shaped workflow document.
    """
    return {"on": {"pull_request": None}, "concurrency": dict(concurrency)}


CONFORMING: typ.Final = _document(
    group="ci-${{ github.ref }}", **{"cancel-in-progress": CANCEL_IN_PROGRESS}
)


def test_the_repository_has_pull_request_workflows() -> None:
    """The swept set is non-empty.

    A contract over a filtered list is satisfied by an empty list, so a
    reader that stopped recognizing the ``pull_request`` trigger would
    report every workflow as conforming. This half fails instead.
    """
    assert PULL_REQUEST_WORKFLOWS, (
        "no workflow was read as pull-request-startable, so the sweep below "
        "asserts nothing; the trigger reader or the workflow directory moved"
    )


@pytest.mark.parametrize("name", sorted(PULL_REQUEST_WORKFLOWS))
def test_pull_request_workflow_cancels_superseded_runs(name: str) -> None:
    """Each pull-request workflow supersedes its own earlier runs."""
    violations = concurrency_violations(PULL_REQUEST_WORKFLOWS[name])
    assert not violations, f"{name} " + "; ".join(violations)


def test_the_conforming_shape_is_accepted() -> None:
    """The shape the repository deploys reports no violation.

    Without this the rejection cases below would pass against a reader
    that refused everything, which would discriminate nothing.
    """
    violations = concurrency_violations(CONFORMING)
    assert not violations, (
        "the deployed shape must report no violation, or the rejection cases "
        f"below would pass against a reader that refused everything: {violations}"
    )


@pytest.mark.parametrize(
    ("document", "fragment"),
    [
        pytest.param({"on": {"pull_request": None}}, "no concurrency", id="absent"),
        pytest.param(
            {"on": {"pull_request": None}, "concurrency": "ci"},
            "not a mapping",
            id="scalar-block",
        ),
        pytest.param(
            _document(group="ci-${{ github.ref }}"),
            "no cancel-in-progress",
            id="cancel-line-removed",
        ),
        pytest.param(
            _document(group="ci-${{ github.ref }}", **{"cancel-in-progress": True}),
            "cancel-in-progress to True",
            id="literal-true",
        ),
        pytest.param(
            _document(group="ci-${{ github.ref }}", **{"cancel-in-progress": "true"}),
            "cancel-in-progress to 'true'",
            id="quoted-true",
        ),
        *(
            pytest.param(
                _document(
                    group=f"ci-${{{{ {value} }}}}",
                    **{"cancel-in-progress": CANCEL_IN_PROGRESS},
                ),
                value,
                id=f"per-run-group-{value.rsplit('.', 1)[-1]}",
            )
            for value in ("github.run_id", "github.run_number", "github.sha")
        ),
        pytest.param(
            _document(group="ci", **{"cancel-in-progress": CANCEL_IN_PROGRESS}),
            "none of github.ref",
            id="static-group",
        ),
        pytest.param(
            _document(**{"cancel-in-progress": CANCEL_IN_PROGRESS}),
            "no concurrency group",
            id="group-absent",
        ),
    ],
)
def test_a_non_conforming_document_is_rejected(
    document: dict[str, object], fragment: str
) -> None:
    """Each way of defeating the rule is reported, and named.

    ``cancel-in-progress: true`` is the case worth stating: it reads as
    an improvement and would cancel a push to main or a scheduled run
    that shares the group. A contract that accepted a truthy value
    would wave it through.
    """
    violations = concurrency_violations(document)
    assert any(fragment in violation for violation in violations), violations


@pytest.mark.parametrize(
    "document",
    [
        pytest.param({"on": {"pull_request": None}}, id="mapping-string-key"),
        pytest.param({True: {"pull_request": None}}, id="mapping-boolean-key"),
        pytest.param({True: ["push", "pull_request"]}, id="sequence"),
        pytest.param({True: "pull_request"}, id="bare-scalar"),
    ],
)
def test_the_trigger_reader_covers_every_accepted_shape(
    document: dict[str, object],
) -> None:
    """An unquoted ``on:`` parses as the boolean True and still reads.

    YAML 1.1 folds ``on`` to ``True``, so a reader that looked only
    under the string key would find no triggers in half the estate's
    workflows and report them as startable by nothing. GitHub accepts a
    mapping, a sequence and a bare scalar, so a reader that handles one
    shape sweeps an incomplete set.
    """
    assert is_pull_request_startable(document), (
        f"{document} declares the pull_request trigger and must read as "
        "pull-request-startable, or the sweep skips it"
    )


@pytest.mark.parametrize(
    "document",
    [
        pytest.param({"on": {"push": None}}, id="push-only"),
        pytest.param({"on": {"pull_request_target": None}}, id="pull-request-target"),
        pytest.param({"on": {"schedule": [{"cron": "0 3 * * *"}]}}, id="schedule"),
    ],
)
def test_a_workflow_no_pull_request_starts_is_out_of_scope(
    document: dict[str, object],
) -> None:
    """Only ``pull_request`` is in scope, and the reader says so.

    ``pull_request_target`` runs with the base repository's token; the
    workflows on it here push commits and merge, so cancelling one
    mid-write is not a saving. A reader that folded the two together
    would widen the rule past what was approved.
    """
    assert not is_pull_request_startable(document), (
        f"{document} declares no pull_request trigger and must stay out of "
        "scope, or the rule widens past what was approved"
    )


def test_a_workflow_with_no_triggers_is_a_shape_fault() -> None:
    """No ``on:`` key is malformed, not "startable by nothing"."""
    with pytest.raises(TriggerShapeError):
        is_pull_request_startable({"jobs": {}})


def test_an_unreadable_trigger_value_is_a_shape_fault() -> None:
    """A trigger key of an unexpected type names the workflow, not Python."""
    with pytest.raises(TriggerShapeError):
        is_pull_request_startable({"on": 42})
