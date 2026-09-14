"""Contract for the scope of ``lading publish``'s pre-flight.

``lading publish`` runs ``cargo check --workspace --all-targets`` and then
``cargo test`` before it packages anything. ``lading.toml`` narrows the
second of those to the library and binary unit tests, which is only
defensible because every lane has already executed the same workspace by
the time it reaches ``make publish-check``.

This module asserts both halves: the setting, and the ordering that
justifies it. Either one alone would be a claim about nothing. Should a
future edit move the publish step above the test steps, the setting stops
being an optimization and becomes a hole, and this fails rather than the
release.

Run via ``make test-workflow-contracts``.
"""

import typing as typ

import pytest
from publish_report_support import DRY_RUN_STEP, step_index, step_named

#: The steps that execute the workspace's tests, one per lane, with the
#: condition that selects the lane each one serves. Every step must
#: precede the publish dry run, and between them the conditions must
#: select every lane the matrix declares: one Linux, and the two Windows
#: legs split on whether `matrix.features` is set.
TEST_STEPS: typ.Final[dict[str, str]] = {
    "Test and Measure Coverage (Linux)": "${{ runner.os == 'Linux' }}",
    "Test and Measure Coverage (Windows, default features)": (
        "${{ runner.os == 'Windows' && matrix.features == '' }}"
    ),
    "Test and Measure Coverage (Windows, strict validation)": (
        "${{ runner.os == 'Windows' && matrix.features != '' }}"
    ),
}


def test_the_preflight_runs_unit_tests_only(
    lading_configuration: dict[str, typ.Any],
) -> None:
    """The pre-flight must not execute the suite a second time.

    Without this the publish step re-runs every test the lane has just
    passed, which measured at 94 percent of the Linux step and 74 to 80
    percent of each Windows step. It also runs the cargo-spawning tests
    outside the nextest test-groups and slow-timeout tiers sized for
    them, so the second run is the less controlled of the two.
    """
    preflight = lading_configuration.get("preflight")
    assert isinstance(preflight, dict), (
        f"lading.toml must declare a [preflight] table, got {preflight!r}"
    )
    assert preflight.get("unit_tests_only") is True, (
        f"lading.toml must set preflight.unit_tests_only = true, got "
        f"{preflight.get('unit_tests_only')!r}; without it the publish step "
        f"runs the whole workspace suite again"
    )


@pytest.mark.parametrize("step_name", tuple(TEST_STEPS), ids=str)
def test_every_lane_tests_before_it_packages(
    build_test_job: dict[str, typ.Any], step_name: str
) -> None:
    """Narrowing the pre-flight is safe only while this holds.

    Each lane's test step is what the narrowed pre-flight relies on. A
    step order that packaged first, or a lane whose test step was
    removed, would leave `unit_tests_only` narrowing the only remaining
    execution of the suite in that job.
    """
    tests_at = step_index(build_test_job, step_name)
    packages_at = step_index(build_test_job, DRY_RUN_STEP)
    assert tests_at < packages_at, (
        f"{step_name!r} runs at index {tests_at}, after the {DRY_RUN_STEP!r} "
        f"step at index {packages_at}; the narrowed pre-flight assumes the "
        f"lane has already executed the workspace"
    )


@pytest.mark.parametrize(("step_name", "condition"), TEST_STEPS.items(), ids=str)
def test_every_lane_is_selected_by_exactly_one_test_step(
    build_test_job: dict[str, typ.Any], step_name: str, condition: str
) -> None:
    """Ordering is not enough on its own: the step must run on that lane.

    A test step that precedes the dry run but whose condition excludes
    the lane leaves that lane packaging without having tested. The three
    conditions are pinned rather than merely counted, because the
    failure that matters is a lane quietly falling outside all of them.
    """
    declared = str(step_named(build_test_job, step_name).get("if", ""))
    assert declared == condition, (
        f"{step_name!r} runs when {declared!r}, not {condition!r}; the lanes "
        f"the dry run packages on must each be tested by one of these steps"
    )
