"""Contract that every step running the workspace suite can fail the job.

A test step marked ``continue-on-error`` reports failures as warnings and
leaves the job green, so the suite runs without gating anything. The two
Windows coverage steps carried that marking, and nobody noticed, because
``lading publish``'s pre-flight happened to run the same tests later in
the same job and failed there instead. Narrowing that pre-flight removed
the accident, which is why the marking is gone and why this exists.

The check is on the whole of ``ci.yml`` rather than on the three steps
that prompted it: the next such step would be added the same way, by
someone keeping a lane green.

Run via ``make test-workflow-contracts``.
"""

import typing as typ

from cache_step_support import runs_workspace_tests
from publish_report_support import CI_WORKFLOW
from workflow_queries import iter_steps

#: Actions that run the workspace suite on the job's behalf. A step using
#: one of these executes the tests even though it declares no ``run``.
TEST_DRIVING_ACTIONS: typ.Final[tuple[str, ...]] = ("generate-coverage",)


def drives_the_workspace_suite(step: dict[str, object]) -> bool:
    """Report whether a step executes the workspace tests.

    Both shapes count: a script invoking a test driver, and a step
    delegating to an action that runs the suite.

    Parameters
    ----------
    step : dict[str, object]
        One workflow step.

    Returns
    -------
    bool
        True when the step runs the workspace tests either way.

    Examples
    --------
    >>> drives_the_workspace_suite({"uses": "leynos/generate-coverage@abc"})
    True
    >>> drives_the_workspace_suite({"uses": "actions/checkout@abc"})
    False
    """
    if runs_workspace_tests(step):
        return True
    uses = str(step.get("uses", ""))
    return any(action in uses for action in TEST_DRIVING_ACTIONS)


def test_no_workspace_test_step_may_fail_without_failing_the_job() -> None:
    """A suite that cannot red the lane is not a gate.

    ``continue-on-error`` turns a failing test run into a warning nobody
    reads. The Windows lanes sat that way for as long as the pre-flight
    masked it: their tests ran, failed nothing, and the job stayed green
    on the strength of a check whose real purpose was publishing.
    """
    ungated = [
        f"{reference.job}: {reference.name}"
        for reference in iter_steps(CI_WORKFLOW)
        if drives_the_workspace_suite(reference.step)
        and reference.step.get("continue-on-error")
    ]

    assert not ungated, (
        f"these steps run the workspace suite but cannot fail the job: "
        f"{ungated}; a test step marked continue-on-error reports failures "
        f"as warnings and gates nothing"
    )


def test_the_contract_has_steps_to_judge() -> None:
    """A finder that matched nothing would pass the check above silently.

    The assertion is a negative one, so it is satisfied by an empty
    workflow, by a renamed action, and by a query that stopped working.
    This pins that the workflow really does contain the steps the
    contract is about.
    """
    found = [
        f"{reference.job}: {reference.name}"
        for reference in iter_steps(CI_WORKFLOW)
        if drives_the_workspace_suite(reference.step)
    ]

    assert len(found) >= 3, (
        f"{CI_WORKFLOW} must declare a workspace test step per lane for the "
        f"gating contract to mean anything; found {found}"
    )
