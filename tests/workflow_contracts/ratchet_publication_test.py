"""Contract for which runs may publish the coverage ratchet baseline.

A ratchet is only a ratchet while the baseline it compares against comes from
somewhere a pull request cannot reach. Before the pinned revision the shared
action published from every run that reached its save step, so a pull request
advanced the baseline it was then measured against, and a ``workflow_dispatch``
replaced whatever generation it was measuring.

None of that is visible from a green run. A ratchet comparing each pull request
against itself passes exactly as one comparing against trunk does, and it goes
on passing while coverage falls.

The pin is therefore asserted by value here, unlike the shape assertions in
``codescene_coverage_test.py``: the guarantee arrived in a particular revision,
so a bump has to update this constant and make someone confirm the new revision
still keeps a pull request from publishing.

This repository needs no opt-in. Its `ci.yml` runs on pushes to `main`, which
is what the action's default guard requires, so the Linux lane leaves
``publish-baseline`` unset and takes it.
"""

import typing as typ
from pathlib import Path

import pytest
import yaml

WORKFLOW_PATH = Path(__file__).resolve().parents[2] / ".github" / "workflows" / "ci.yml"

#: The revision that guards the baseline save on a push to refs/heads/main.
GENERATE_COVERAGE = (
    "leynos/shared-actions/.github/actions/generate-coverage@"
    "77ea10341249024e22ec5d9069e3caa7596e0d4f"
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


def test_every_coverage_step_shares_the_guarded_revision(
    coverage_steps: list[dict[str, typ.Any]],
) -> None:
    """One pin, and it must be the one that carries the guard.

    Two lanes on different revisions would be worse than one stale pin: the
    behaviour would depend on which lane a reader happened to check.
    """
    pinned = {str(step.get("uses")) for step in coverage_steps}

    assert pinned == {GENERATE_COVERAGE}, (
        f"every coverage step must be pinned to {GENERATE_COVERAGE}, found "
        f"{sorted(pinned)}"
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
    """Leaving ``publish-baseline`` unset is what keeps a pull request out.

    Setting it to ``always`` would restore exactly the behaviour this pin
    exists to remove. Asserted over every ratcheting lane, so a lane added
    later cannot quietly become a second writer.
    """
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
    """The guard publishes on a push to main, so that trigger must exist.

    Without it no run could ever publish, and the ratchet would compare every
    pull request against a baseline that had stopped advancing.
    """
    # PyYAML parses the bare `on:` key as the boolean True.
    triggers = workflow.get("on", workflow.get(True))
    assert isinstance(triggers, dict), "the workflow must declare an on: mapping"

    assert trigger in triggers, f"the workflow must still run on {trigger}"


def test_trunk_pushes_are_restricted_to_main(
    workflow: dict[str, typ.Any],
) -> None:
    """The guard names refs/heads/main, so the trigger must not widen it."""
    triggers = workflow.get("on", workflow.get(True))
    assert isinstance(triggers, dict), "the workflow must declare an on: mapping"
    push = triggers["push"]
    assert isinstance(push, dict), "the push trigger must declare branches"

    assert push.get("branches") == ["main"], (
        f"the push trigger must be restricted to main, got {push.get('branches')!r}"
    )
