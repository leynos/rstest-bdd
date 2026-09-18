"""CV-005: main owns every CodeScene interaction.

A pull-request lane measures coverage for its own ratchet and does nothing
else with it. It carries no CodeScene action, no ``cs-coverage`` command and
no ``CS_ACCESS_TOKEN``. One workflow, reachable only from a push to ``main``
or a dispatch, generates the trunk report and uploads it.

The separation is not tidiness. Between 2026-09-16 and 2026-09-18 an unpinned
``cs-coverage`` could not parse its own cobertura output, and because the
check ran inside the merge gate every pull request in this repository was
blocked on a step with nothing to say about the change under review. A
pull-request lane that cannot contact CodeScene cannot be stopped by
CodeScene.

Both lanes must measure the same thing, or the baseline the trunk writes is
not the baseline the ratchet should compare against. The inputs are held
equal here rather than trusted to have been copied correctly.

Run via ``make test-workflow-contracts``.
"""

import pytest
from codescene_coverage_support import (
    DEPRECATED_DIGEST_VARIABLE,
    MARKER_FIXTURES,
    MARKERS,
    PR_COVERAGE_STEP,
    PR_WORKFLOW,
    PUBLISHER,
    PUBLISHER_COVERAGE_STEP,
    PULL_REQUEST_EVENTS,
    SELECTION_INPUTS,
    SHA_PINNED,
    _walk,
    codescene_references,
    coverage_step,
    pull_request_workflows,
    references_in,
    triggers,
)
from workflow_queries import iter_steps, workflow_names
from workflow_support import workflow


@pytest.mark.parametrize("marker", sorted(MARKERS))
def test_each_marker_finds_its_own_interaction(marker: str) -> None:
    """Prove every marker separately.

    The scan clears a workflow by finding nothing, so a marker that had
    stopped matching would clear the very thing it exists to catch, and every
    other marker would keep the suite green. Each is therefore driven over a
    document carrying only its own interaction.
    """
    found = references_in(MARKER_FIXTURES[marker], "fixture.yml")

    assert any(entry.startswith(marker) for entry in found), (
        f"{marker} must be recognized; the scan of its own fixture found {found}"
    )


def test_a_workflow_free_of_codescene_is_cleared() -> None:
    """Prove the scan can clear as well as refuse.

    A scanner that reported an interaction in every document would pass its
    marker tests and fail every real workflow, which is the opposite failure
    and just as invisible from the rules above.
    """
    innocent = {"jobs": {"gate": {"steps": [{"run": "make test"}]}}}

    found = references_in(innocent, "fixture.yml")

    assert found == [], (
        "a workflow that names no CodeScene action, runs no cs-coverage "
        f"command and carries no credential must be cleared; found {found}"
    )


def test_the_scan_reaches_the_workflows_it_guards() -> None:
    """Refuse a vacuous traversal.

    Every rule below rests on the traversal finding workflows. One that found
    none would report that no pull-request workflow contacts CodeScene while
    asserting nothing at all.
    """
    reachable = pull_request_workflows()

    assert PR_WORKFLOW in reachable, (
        f"the scan must reach {PR_WORKFLOW}; it reached {reachable}"
    )
    assert codescene_references(PUBLISHER), (
        f"{PUBLISHER} must contact CodeScene, and the scanner must say so"
    )


def test_no_pull_request_workflow_contacts_codescene() -> None:
    """Keep CodeScene out of everything a pull request can reach.

    Not merely out of the merge gate's coverage step: a credential on any
    lane a pull request's head can reach is the exposure, and an outage in
    any such lane is a block the change under review cannot clear.
    """
    offending = {
        name: codescene_references(name)
        for name in pull_request_workflows()
        if codescene_references(name)
    }

    assert not offending, (
        "no workflow reachable from a pull request may name a CodeScene "
        "action, run cs-coverage, or carry CS_ACCESS_TOKEN; main owns every "
        f"CodeScene interaction (CV-005). Found: {offending}"
    )


def test_no_workflow_reads_the_deprecated_cli_digest() -> None:
    """Leave no workflow maintaining or reading a value nothing consumes.

    `CODESCENE_CLI_SHA256` held the digest of the CodeScene installer script,
    and `installer-checksum` was its only consumer. From shared-actions
    `f68e8e2e` that input is rejected when non-empty and the CLI is pinned
    through the action's own manifest instead, so a workflow still reading the
    variable is feeding a rejected input, and one still refreshing it is
    maintaining a value nothing reads. Neither fails loudly on its own, which
    is why this is asserted rather than left to be noticed.
    """
    offending = sorted(
        f"{name}: {path}"
        for name in workflow_names()
        for path, text in _walk(workflow(name), name)
        if DEPRECATED_DIGEST_VARIABLE in text
    )

    assert not offending, (
        f"no workflow may read or refresh {DEPRECATED_DIGEST_VARIABLE}; the "
        f"shared action pins the CLI through its own manifest: {offending}"
    )


def test_no_caller_passes_the_deprecated_installer_checksum() -> None:
    """Refuse the input the shared action now rejects.

    From shared-actions f68e8e2e the CodeScene CLI is pinned through a
    manifest and a non-empty ``installer-checksum`` fails the run outright.
    The replacement is ``archive-checksum``. Passing the old input is a red
    lane, not a deprecation warning.
    """
    offending = [
        str(reference)
        for name in workflow_names()
        for reference in iter_steps(name)
        if isinstance(inputs := reference.step.get("with"), dict)
        and inputs.get("installer-checksum")
    ]

    assert not offending, (
        "installer-checksum is rejected when non-empty; use archive-checksum: "
        f"{offending}"
    )


def test_every_shared_coverage_action_is_sha_pinned() -> None:
    """Pin both shared actions to a full commit SHA."""
    offending = [
        f"{reference}: {reference.uses}"
        for name in workflow_names()
        for reference in iter_steps(name)
        if (
            "generate-coverage@" in reference.uses
            or "upload-codescene-coverage@" in reference.uses
        )
        and not SHA_PINNED.fullmatch(reference.uses)
    ]

    assert not offending, (
        f"shared coverage actions must be pinned to a full SHA: {offending}"
    )


def test_both_lanes_call_one_coverage_revision() -> None:
    """Hold the merge gate and the publisher to one action revision.

    `ratchet_publication_test` makes this claim about `ci.yml` alone, which
    was the whole story while one workflow generated coverage. It is not now:
    the publisher could be repinned by itself, and the baseline it wrote would
    then come from a different implementation than the one measuring the pull
    request compared against it. Dependabot owns the value; this owns the
    agreement.
    """
    revisions = {
        reference.uses
        for name in workflow_names()
        for reference in iter_steps(name)
        if "generate-coverage@" in reference.uses
    }

    assert len(revisions) == 1, (
        "every generate-coverage call in the repository must use one "
        f"revision; found {sorted(revisions)}"
    )


def test_only_the_publisher_measures_coverage_on_the_trunk() -> None:
    """Leave one baseline writer on a push to main.

    The shared action publishes a baseline only on a push to
    ``refs/heads/main``. Two lanes generating ratcheted coverage on that
    event would therefore both publish, and a pull request would be measured
    against whichever finished last.
    """
    step = coverage_step(PR_WORKFLOW, PR_COVERAGE_STEP)
    guard = str(
        next(
            reference.step.get("if", "")
            for reference in iter_steps(PR_WORKFLOW)
            if reference.name == PR_COVERAGE_STEP
        )
    )

    assert step.get("with-ratchet") == "true", (
        f"{PR_COVERAGE_STEP} must compare against the trunk baseline"
    )
    assert "github.event_name == 'pull_request'" in guard, (
        f"{PR_COVERAGE_STEP} must run on pull requests only, or it becomes a "
        f"second baseline writer on the trunk; its guard is {guard!r}"
    )


def test_the_merge_gate_runs_no_coverage_on_the_trunk() -> None:
    """Leave the trunk one run of the workspace suite.

    Every coverage step here is also this repository's test execution for its
    lane, so a step without the event clause runs the suite on a push as well,
    and the trunk then tests the same commit two or three times over. It is
    also a second baseline writer: the shared action publishes on a push to
    `refs/heads/main`, so a ratcheting step reaching that event would race the
    publisher. The rule is the whole set, not the Linux lane alone.
    """
    offending = [
        f"{reference}: {reference.step.get('if')!r}"
        for reference in iter_steps(PR_WORKFLOW)
        if "generate-coverage@" in reference.uses
        and "github.event_name == 'pull_request'" not in str(reference.step.get("if"))
    ]

    assert not offending, (
        f"every coverage step in {PR_WORKFLOW} must run on pull requests "
        f"only; the trunk's run belongs to {PUBLISHER}: {offending}"
    )


def test_the_trigger_reader_survives_the_boolean_on_key() -> None:
    """Refuse the reading that voids every rule resting on triggers.

    YAML 1.1 parses the bare word ``on`` as the boolean ``True``, so a
    workflow's trigger block is not under a string key at all. A reader that
    asked for ``"on"`` would find nothing, conclude that no workflow declares
    a pull-request trigger, and report a clean estate while checking none of
    it. The hazard is named here so a later simplification fails.
    """
    document = workflow(PR_WORKFLOW)

    assert True in document, "a workflow's trigger block is keyed under True"
    assert "on" not in document, (
        "nothing is keyed under the string 'on'; reading it would make every "
        "trigger rule here pass on an empty mapping"
    )
    assert PULL_REQUEST_EVENTS & set(triggers(PR_WORKFLOW)), (
        f"{PR_WORKFLOW} must declare a pull-request trigger, or the scan that "
        "clears pull-request workflows is clearing an empty set"
    )


def test_the_pull_request_lane_declines_publication() -> None:
    """Keep report publication to the workflow that owns it.

    The action archives the report it generated under a step of its own,
    which no scanner over this workflow's steps can see. Declining the
    archive explicitly is the only way the boundary is observable here.
    """
    inputs = coverage_step(PR_WORKFLOW, PR_COVERAGE_STEP)

    assert inputs.get("publish-artefact") == "false", (
        f"{PR_COVERAGE_STEP} must decline the artefact; publication belongs "
        f"to {PUBLISHER}. It declares {inputs.get('publish-artefact')!r}"
    )


@pytest.mark.parametrize("field", SELECTION_INPUTS)
def test_both_lanes_measure_the_same_thing(field: str) -> None:
    """Hold the trunk generation and the pull-request check to one selection.

    The ratchet compares a pull request's changed-line coverage against the
    baseline this repository's trunk wrote. If the two lanes select different
    features, targets or test drivers, that comparison is between two
    different measurements and a pull request can fail or pass on the
    difference rather than on its own change.
    """
    gate = coverage_step(PR_WORKFLOW, PR_COVERAGE_STEP)
    publisher = coverage_step(PUBLISHER, PUBLISHER_COVERAGE_STEP)

    assert gate.get(field) == publisher.get(field), (
        f"{PR_COVERAGE_STEP} selects {field}={gate.get(field)!r} and "
        f"{PUBLISHER_COVERAGE_STEP} selects {publisher.get(field)!r}; the "
        "baseline would not be comparable with what the ratchet checks"
    )


def test_the_selection_list_covers_every_input_both_lanes_declare() -> None:
    """Refuse a selection list that has fallen behind the workflows.

    The list above is named, so an input added to both lanes and not to it
    would simply not be compared, and the two could then drift on it.
    """
    gate = set(coverage_step(PR_WORKFLOW, PR_COVERAGE_STEP))
    publisher = set(coverage_step(PUBLISHER, PUBLISHER_COVERAGE_STEP))
    shared = gate & publisher

    assert shared == set(SELECTION_INPUTS), (
        "every input both lanes declare must be compared; unlisted: "
        f"{sorted(shared - set(SELECTION_INPUTS))}, listed but absent: "
        f"{sorted(set(SELECTION_INPUTS) - shared)}"
    )
