"""CV-005: the one step allowed to contact CodeScene, and how it is reached.

`codescene_coverage_test` clears everything a pull request can run. These
rules hold the other side: which events reach the publisher, exactly what its
upload step sends, the conditions under which it runs, and exactly where the
credential appears. Each is a way the estate rule fails while every
pull-request lane stays clean.

The run conditions are evaluated as GitHub would evaluate them, for a push to
`main`, a dispatch from another ref, and a repository without the token,
rather than searched for a phrase: a guard with `|| github.event_name ==
'workflow_dispatch'` appended still contains the main-ref clause.

Run via ``make test-workflow-contracts``.
"""

import pytest
from codescene_coverage_support import PUBLISHER, PUBLISHER_JOB, _walk, triggers
from guard_conditions import admits, conjuncts
from pull_request_reach import trigger_names
from workflow_queries import StepRef, iter_steps
from workflow_support import workflow

MAIN_REF = "github.ref == 'refs/heads/main'"
#: Everything the upload sends, stated whole. A partial check would pass an
#: upload that lost its report path or its credential and published nothing.
UPLOAD_INPUTS = {
    "path": "coverage.xml",
    "format": "cobertura",
    "mode": "upload",
    "access-token": "${{ env.CS_ACCESS_TOKEN }}",
}


def _upload() -> tuple[int, StepRef]:
    """Return the publisher's one upload step and its position in its job.

    Returns
    -------
    tuple[int, StepRef]
        The step's index within its job's steps, and the step.
    """
    uploads = [
        (index, reference)
        for index, reference in enumerate(
            reference
            for reference in iter_steps(PUBLISHER)
            if reference.job == PUBLISHER_JOB
        )
        if "upload-codescene-coverage@" in reference.uses
    ]
    everywhere = [
        str(reference)
        for reference in iter_steps(PUBLISHER)
        if "upload-codescene-coverage@" in reference.uses
    ]
    assert len(uploads) == 1, f"{PUBLISHER_JOB} must upload exactly once"
    assert len(everywhere) == 1, f"{PUBLISHER} must upload once; {everywhere}"
    return uploads[0]


def test_the_publisher_answers_a_push_to_main_and_a_dispatch_only() -> None:
    """Hold the publisher's triggers to exactly the two it needs.

    No pull-request event, because this workflow holds the credential. And
    both of the others: without the push it never publishes, and without the
    dispatch there is no way to measure the lane without merging.
    """
    declared = trigger_names(workflow(PUBLISHER), PUBLISHER)
    push = triggers(PUBLISHER).get("push")

    assert declared == {"push", "workflow_dispatch"}, (
        f"{PUBLISHER} must trigger on a push and a dispatch only; it declares "
        f"{sorted(declared)}"
    )
    assert isinstance(push, dict), f"{PUBLISHER} must filter its push trigger"
    assert push.get("branches") == ["main"], (
        f"{PUBLISHER} must be restricted to pushes to main; got "
        f"{push.get('branches')!r}"
    )


def test_the_upload_sends_exactly_the_trunk_report() -> None:
    """Upload the trunk report whole; never gate on it from here.

    ``mode: check`` is the pull-request form, and the form that failed. The
    rest is stated too, because an upload with no path, a different format,
    or no credential passes a mode check and publishes nothing.
    """
    _index, reference = _upload()

    assert reference.step.get("with") == UPLOAD_INPUTS, (
        f"{reference} must send {UPLOAD_INPUTS}; it sends {reference.step.get('with')}"
    )


def test_the_upload_guard_is_a_conjunction_holding_the_main_ref() -> None:
    """Require the main-ref clause as a conjunct, and refuse any disjunction.

    `workflow_dispatch` can select any branch or tag, and the push trigger's
    branch filter says nothing about a dispatch, so only this clause stops a
    dispatch from a feature branch publishing that branch as the trunk.
    """
    _index, reference = _upload()

    assert MAIN_REF in conjuncts(str(reference.step.get("if", ""))), (
        f"{reference} must carry {MAIN_REF!r} as a conjunct"
    )


@pytest.mark.parametrize(
    ("event", "ref", "token", "expected"),
    [
        ("push", "refs/heads/main", "set", True),
        ("workflow_dispatch", "refs/heads/main", "set", True),
        ("workflow_dispatch", "refs/heads/feature", "set", False),
        ("workflow_dispatch", "refs/tags/v1.0.0", "set", False),
        ("push", "refs/heads/main", "", False),
    ],
    ids=[
        "push to main",
        "dispatch on main",
        "dispatch on a branch",
        "dispatch on a tag",
        "no token",
    ],
)
def test_the_upload_runs_only_for_the_trunk(
    event: str, ref: str, token: str, expected: object
) -> None:
    """Evaluate the upload's guard for each way the workflow can be started."""
    _index, reference = _upload()
    context = {
        "github.event_name": event,
        "github.ref": ref,
        "env.CS_ACCESS_TOKEN": token,
    }

    assert admits(str(reference.step.get("if", "")), context) is expected, (
        f"{reference} must {'run' if expected else 'not run'} for a {event} "
        f"on {ref} {'with' if token else 'without'} the token"
    )


def test_the_credential_appears_only_where_the_upload_needs_it() -> None:
    """Hold every mention of the token to the upload and its job's scope.

    "Some step has it and no other job has it" proves nothing: the token
    could move to the coverage-generation step, or up to the workflow's own
    `env`, which reaches every step of every job, and such a rule would
    still pass. The set of places is stated exactly: the job scope that the
    upload's guard reads, the guard itself, and the input that sends it.
    """
    index, _reference = _upload()
    step = f"{PUBLISHER}.jobs.{PUBLISHER_JOB}.steps[{index}]"
    expected = {
        f"{PUBLISHER}.jobs.{PUBLISHER_JOB}.env.CS_ACCESS_TOKEN",
        f"{step}.if",
        f"{step}.with.access-token",
    }

    found = {
        path
        for path, text in _walk(workflow(PUBLISHER), PUBLISHER)
        if "CS_ACCESS_TOKEN" in text
    }

    assert found == expected, (
        f"the CodeScene credential may appear only at {sorted(expected)}; "
        f"it appears at {sorted(found)}"
    )
