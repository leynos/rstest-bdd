"""Prove the `codescene` environment sits on the uploading job alone.

Each test mutates a copy of this repository's workflows the way a later edit
could, and asserts the clause meant to catch it does. The check step, the ref
guard and `access-token:` stay held by the existing CV-005 contract.
"""

import copy
import typing as typ
from pathlib import Path

import pytest
from codescene_environment_rules import (
    MISSING,
    REACHABLE,
    STRAY,
    UPLOAD_ACTION,
    Workflow,
    environment_violations,
    jobs,
    pull_request_closure,
    read_workflows,
)

WORKFLOWS: typ.Final[Path] = (
    Path(__file__).resolve().parents[2] / ".github" / "workflows"
)
PUBLISHER: typ.Final[str] = "coverage-main.yml"
LANE: typ.Final[str] = "ci.yml"


@pytest.fixture
def workflows() -> dict[str, Workflow]:
    """Return a private copy of the repository's workflows to mutate.

    Returns
    -------
    dict[str, Workflow]
        The parsed workflows, read afresh for this test alone.

    """
    return copy.deepcopy(read_workflows(WORKFLOWS))


def _first_job(workflows: dict[str, Workflow], name: str) -> dict[object, object]:
    """Return one workflow's first job, for mutation in place.

    Returns
    -------
    dict[object, object]
        The job mapping.

    """
    return next(iter(jobs(workflows[name]).values()))


def _reports(workflows: dict[str, Workflow], fragment: str) -> None:
    """Fail unless the rule reports a violation containing `fragment`."""
    found = environment_violations(workflows)
    assert any(fragment in problem for problem in found), (
        f"expected a violation naming {fragment!r}, got {found}"
    )


def test_repository_places_the_environment(workflows: dict[str, Workflow]) -> None:
    """The publisher declares the environment and nothing else does."""
    found = environment_violations(workflows)
    assert not found, f"expected no violations, got {found}"


def test_the_pull_request_lane_is_read(workflows: dict[str, Workflow]) -> None:
    """The closure reaches the lane, so the third clause has something to read."""
    reached = pull_request_closure(workflows)
    assert LANE in reached, f"{LANE} must be read as pull-request reachable: {reached}"


def test_publisher_cannot_drop_the_environment(workflows: dict[str, Workflow]) -> None:
    """Without it the moved token never reaches the upload, which then skips."""
    del _first_job(workflows, PUBLISHER)["environment"]
    _reports(workflows, MISSING)


def test_publisher_cannot_name_another_environment(
    workflows: dict[str, Workflow],
) -> None:
    """Another environment holds no CodeScene token."""
    _first_job(workflows, PUBLISHER)["environment"] = "production"
    _reports(workflows, MISSING)


def test_mapping_form_is_accepted(workflows: dict[str, Workflow]) -> None:
    """`{name: codescene}` is the same declaration as the bare string."""
    _first_job(workflows, PUBLISHER)["environment"] = {"name": "codescene"}
    found = environment_violations(workflows)
    assert not found, f"the mapping form must be accepted, got {found}"


def test_no_other_job_may_declare_it(workflows: dict[str, Workflow]) -> None:
    """A second holder of the token widens what can read it."""
    declared = typ.cast("dict[object, object]", workflows[PUBLISHER]["jobs"])
    declared["other"] = {
        "runs-on": "ubuntu-latest",
        "environment": "codescene",
        "steps": [{"run": "true"}],
    }
    _reports(workflows, STRAY)


def test_no_pull_request_job_may_declare_it(workflows: dict[str, Workflow]) -> None:
    """A pull request's own code must never be able to request the token."""
    _first_job(workflows, LANE)["environment"] = {"name": "codescene"}
    _reports(workflows, REACHABLE)


def test_a_called_workflow_is_read_too(workflows: dict[str, Workflow]) -> None:
    """A workflow the lane calls runs for the pull request as well."""
    workflows["called.yml"] = {
        "on": {"workflow_call": None},
        "jobs": {"inner": {"environment": "codescene", "steps": [{"run": "true"}]}},
    }
    declared = typ.cast("dict[object, object]", workflows[LANE]["jobs"])
    declared["forward"] = {"uses": "./.github/workflows/called.yml"}
    _reports(workflows, f"called.yml:inner {REACHABLE}")


def test_an_empty_reading_is_refused(workflows: dict[str, Workflow]) -> None:
    """With no uploader left the rule says so rather than passing."""
    job = _first_job(workflows, PUBLISHER)
    job["steps"] = [
        step
        for step in typ.cast("list[dict[object, object]]", job["steps"])
        if UPLOAD_ACTION not in str(step.get("uses", ""))
    ]
    _reports(workflows, "no workflow job invokes")
