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

The token is in no `env`. The upload is a composite action whose nested steps
inherit the calling step's environment, so a check step publishes only whether
the token exists, the guard reads that output, and the upload takes the token
directly as an input.

Run via ``make test-workflow-contracts``.
"""

import pytest
from codescene_coverage_support import PUBLISHER, PUBLISHER_JOB, _walk, triggers
from guard_conditions import admits, conjuncts
from pull_request_reach import trigger_names
from workflow_queries import StepRef, iter_steps
from workflow_support import workflow

MAIN_REF = "github.ref == 'refs/heads/main'"
CHECK_STEP_ID = "codescene_token"
#: GitHub evaluates the expression before the shell starts, so the command
#: writes a literal ``true`` or ``false`` and the token enters no process.
CHECK_COMMAND = (
    'echo "available=${{ secrets.CS_ACCESS_TOKEN != \'\' }}" >> "$GITHUB_OUTPUT"'
)
AVAILABLE = f"steps.{CHECK_STEP_ID}.outputs.available"
#: Expression contexts are case-insensitive, so the credential scan folds case.
CREDENTIAL_NAME = "cs_access_token"
#: Everything the upload sends, stated whole. A partial check would pass an
#: upload that lost its report path or its credential and published nothing.
UPLOAD_INPUTS = {
    "path": "coverage.xml",
    "format": "cobertura",
    "mode": "upload",
    "access-token": "${{ secrets.CS_ACCESS_TOKEN }}",
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


def _check() -> tuple[int, StepRef]:
    """Return the publisher job's availability check and its position.

    Returns
    -------
    tuple[int, StepRef]
        The step's index within its job's steps, and the step.
    """
    checks = [
        (index, reference)
        for index, reference in enumerate(
            reference
            for reference in iter_steps(PUBLISHER)
            if reference.job == PUBLISHER_JOB
        )
        if reference.step.get("id") == CHECK_STEP_ID
    ]
    assert len(checks) == 1, f"{PUBLISHER_JOB} must carry one {CHECK_STEP_ID} step"
    return checks[0]


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


def test_the_check_publishes_availability_and_nothing_else() -> None:
    """Run one exact command, unconditionally, with no env, before the upload.

    A condition on the check would leave its output unset whenever it was
    false, so the upload would skip forever, and an ``env`` would put the
    token back into an environment.
    """
    index, reference = _check()
    upload_index, _upload_reference = _upload()

    assert str(reference.step.get("run", "")).strip() == CHECK_COMMAND, (
        f"{reference} must run exactly {CHECK_COMMAND!r}"
    )
    assert "if" not in reference.step, f"{reference} must run unconditionally"
    assert "env" not in reference.step, f"{reference} must declare no env"
    assert index < upload_index, f"{reference} must run before the upload"


def test_the_upload_guard_reads_the_check() -> None:
    """Require the check's output as a conjunct of the upload's guard."""
    _index, reference = _upload()
    required = f"{AVAILABLE} == 'true'"

    assert required in conjuncts(str(reference.step.get("if", ""))), (
        f"{reference} must carry {required!r} as a conjunct"
    )


@pytest.mark.parametrize(
    ("event", "ref", "token", "expected"),
    [
        ("push", "refs/heads/main", "true", True),
        ("workflow_dispatch", "refs/heads/main", "true", True),
        ("workflow_dispatch", "refs/heads/feature", "true", False),
        ("workflow_dispatch", "refs/tags/v1.0.0", "true", False),
        ("push", "refs/heads/main", "false", False),
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
        AVAILABLE: token,
    }

    assert admits(str(reference.step.get("if", "")), context) is expected, (
        f"{reference} must {'run' if expected else 'not run'} for a {event} "
        f"on {ref} with the check reporting {token}"
    )


def test_the_credential_appears_only_where_the_upload_needs_it() -> None:
    """Hold every mention of the token to the check and the upload's input.

    "No env has it" proves nothing on its own: deleting the token satisfies
    it while the guard goes false and the upload skips forever. The set of
    places is stated exactly, keys included and case folded: the check's
    command, and the input that sends the token.
    """
    check_index, _check_reference = _check()
    upload_index, _reference = _upload()
    steps_path = f"{PUBLISHER}.jobs.{PUBLISHER_JOB}.steps"
    expected = {
        f"{steps_path}[{check_index}].run",
        f"{steps_path}[{upload_index}].with.access-token",
    }

    found = {
        path
        for path, text in _walk(workflow(PUBLISHER), PUBLISHER)
        if CREDENTIAL_NAME in text.casefold()
    }

    assert found == expected, (
        f"the CodeScene credential may appear only at {sorted(expected)}; "
        f"it appears at {sorted(found)}"
    )
