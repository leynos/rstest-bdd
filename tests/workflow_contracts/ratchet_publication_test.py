"""Contract for the caller's coverage-ratchet workflow configuration.

The local workflow must invoke one SHA-pinned shared coverage action revision,
enable the ratchet in its designated lane, and let the action apply its default
publication guard. It must also run on pull requests and pushes to ``main`` so
the workflow supplies the events that the local caller contract requires.

Dependabot owns the shared-action revision. This test verifies the local
invocation shape and cross-lane consistency without making a claim about the
implementation of any particular remote action revision.
"""

import re
import typing as typ
from pathlib import Path

import pytest
import yaml

WORKFLOW_PATH = Path(__file__).resolve().parents[2] / ".github" / "workflows" / "ci.yml"

GENERATE_COVERAGE_USES_RE = re.compile(
    r"^leynos/shared-actions/\.github/actions/generate-coverage@[0-9a-f]{40}$"
)

#: The lane that enables the ratchet, and so the only one the guard governs.
RATCHET_LANE = "Test and Measure Coverage (Linux)"


@pytest.fixture(scope="module")
def workflow() -> dict[str, typ.Any]:
    """Return the parsed workflow, read once for the module."""
    document = yaml.safe_load(WORKFLOW_PATH.read_text(encoding="utf-8"))
    assert isinstance(document, dict), "the workflow must parse to a mapping"
    return document


@pytest.fixture(scope="module")
def coverage_steps(workflow: dict[str, typ.Any]) -> list[dict[str, typ.Any]]:
    """Return every step invoking the shared coverage action."""
    jobs = workflow.get("jobs")
    assert isinstance(jobs, dict), "the workflow must declare a jobs mapping"
    found = [
        step
        for definition in jobs.values()
        if isinstance(definition, dict)
        for step in definition.get("steps") or []
        if isinstance(step, dict)
        and "generate-coverage@" in str(step.get("uses") or "")
    ]
    assert found, "the workflow must invoke the shared coverage action"
    return found


def _ratcheting_steps(
    coverage_steps: list[dict[str, typ.Any]],
) -> dict[str, dict[str, typ.Any]]:
    """Return every coverage step that enables the ratchet, keyed by name."""
    return {
        str(step.get("name")): step
        for step in coverage_steps
        if isinstance(step.get("with"), dict)
        and step["with"].get("with-ratchet") == "true"
    }


def test_every_coverage_step_is_sha_pinned_and_uses_one_revision(
    coverage_steps: list[dict[str, typ.Any]],
) -> None:
    """All coverage calls use one full-SHA pin managed by Dependabot.

    Multiple revisions would make the local workflow's behaviour depend on the
    lane, while a tag, branch, or abbreviated SHA would permit a mutable or
    ambiguous action reference.
    """
    references = [str(step.get("uses")) for step in coverage_steps]
    non_sha_pinned = [
        reference
        for reference in references
        if not GENERATE_COVERAGE_USES_RE.fullmatch(reference)
    ]
    assert not non_sha_pinned, (
        "coverage steps must use the generate-coverage action pinned to a full "
        f"lowercase 40-character SHA; non-SHA-pinned references: {non_sha_pinned}"
    )
    pinned = set(references)
    assert len(pinned) == 1, (
        "coverage steps must use one consistent generate-coverage reference; "
        f"inconsistent references: {sorted(pinned)}"
    )


def test_one_lane_ratchets_and_it_is_the_documented_one(
    coverage_steps: list[dict[str, typ.Any]],
) -> None:
    """Only one step may write the baseline, and it must be the known one.

    Checking the expected lane alone would prove only that it exists. A second
    lane enabling the ratchet would be another baseline writer, and could opt
    itself out of the guard without this contract noticing.
    """
    ratcheting = _ratcheting_steps(coverage_steps)

    assert set(ratcheting) == {RATCHET_LANE}, (
        f"exactly one coverage step may enable the ratchet, and it must be "
        f"{RATCHET_LANE!r}; found {sorted(ratcheting)}"
    )


def test_no_ratcheting_lane_opts_out_of_the_guard(
    coverage_steps: list[dict[str, typ.Any]],
) -> None:
    """The caller leaves baseline-publication policy to the shared action."""
    for name, step in _ratcheting_steps(coverage_steps).items():
        inputs = step["with"]
        assert "publish-baseline" not in inputs, (
            f"{name} sets publish-baseline={inputs.get('publish-baseline')!r}; "
            f"a pull request would then advance the baseline it is measured "
            f"against"
        )


@pytest.mark.parametrize("trigger", ["push", "pull_request"])
def test_the_workflow_still_runs_where_the_guard_expects_it(
    workflow: dict[str, typ.Any], trigger: str
) -> None:
    """The local caller must keep its pull-request and push triggers."""
    # PyYAML parses the bare `on:` key as the boolean True.
    triggers = workflow.get("on", workflow.get(True))
    assert isinstance(triggers, dict), "the workflow must declare an on: mapping"

    assert trigger in triggers, f"the workflow must still run on {trigger}"


def test_trunk_pushes_are_restricted_to_main(
    workflow: dict[str, typ.Any],
) -> None:
    """The local caller publishes its trunk generation only from ``main``."""
    triggers = workflow.get("on", workflow.get(True))
    assert isinstance(triggers, dict), "the workflow must declare an on: mapping"
    push = triggers["push"]
    assert isinstance(push, dict), "the push trigger must declare branches"

    assert push.get("branches") == ["main"], (
        f"the push trigger must be restricted to main, got {push.get('branches')!r}"
    )
