"""CV-005: the shape of the workflow that owns CodeScene publication.

`codescene_coverage_test` clears the lanes a pull request can reach. These
rules are the other half: what the one workflow allowed to hold the credential
must look like. Its triggers, the mode it uploads in, the ref it publishes
from, and whether two trunk generations can overlap are each a way the estate
rule fails while every pull-request lane stays clean.

Run via ``make test-workflow-contracts``.
"""

from codescene_coverage_support import (
    PUBLISHER,
    PUBLISHER_COVERAGE_STEP,
    PULL_REQUEST_EVENTS,
    coverage_step,
    triggers,
)
from workflow_queries import iter_steps
from workflow_support import workflow


def test_the_publisher_is_not_reachable_from_a_pull_request() -> None:
    """Keep the workflow that holds the credential off pull-request events."""
    declared = set(triggers(PUBLISHER))

    assert not PULL_REQUEST_EVENTS & declared, (
        f"{PUBLISHER} holds the CodeScene credential, so it must not trigger "
        f"on a pull request; it declares {sorted(declared)}"
    )
    assert declared <= {"push", "workflow_dispatch"}, (
        f"{PUBLISHER} may trigger only on a push or a dispatch; it declares "
        f"{sorted(declared)}"
    )
    push = triggers(PUBLISHER)["push"]
    assert isinstance(push, dict), (
        f"{PUBLISHER} must filter its push trigger; got {push!r}"
    )
    assert push.get("branches") == ["main"], (
        f"{PUBLISHER} must be restricted to pushes to main; got "
        f"{push.get('branches')!r}"
    )


def test_the_publisher_uploads_rather_than_checks() -> None:
    """Upload the trunk report; never gate on it from here.

    ``mode: check`` is the pull-request form, and the form that failed. This
    lane publishes an analysed branch's report, which is what the ratchet
    baseline and the CodeScene project both read.
    """
    uploads = [
        reference
        for reference in iter_steps(PUBLISHER)
        if "upload-codescene-coverage@" in reference.uses
    ]

    assert uploads, f"{PUBLISHER} must upload the trunk report to CodeScene"
    for reference in uploads:
        inputs = reference.step.get("with")
        assert isinstance(inputs, dict), f"{reference} must declare inputs"
        assert inputs.get("mode") == "upload", (
            f"{reference} must upload, not {inputs.get('mode')!r}"
        )


def test_the_upload_is_restricted_to_the_main_ref() -> None:
    """Publish only from `main`, whatever the event selected.

    `workflow_dispatch` can select any branch or tag. The push trigger's
    `branches: [main]` says nothing about a dispatch, so without a ref guard
    on the step itself a dispatch from a feature branch publishes that
    branch's coverage through the main-owned upload, and CodeScene records it
    as the trunk's. The shared action already holds the ratchet baseline to a
    push to `refs/heads/main`; this brings the report under the same rule.
    """
    uploads = [
        reference
        for reference in iter_steps(PUBLISHER)
        if "upload-codescene-coverage@" in reference.uses
    ]

    assert uploads, f"{PUBLISHER} must upload the trunk report to CodeScene"
    for reference in uploads:
        guard = str(reference.step.get("if", ""))
        assert "github.ref == 'refs/heads/main'" in guard, (
            f"{reference} must publish only from main; its guard is {guard!r}"
        )


def test_the_publisher_serializes_its_trunk_generations() -> None:
    """Let one trunk generation finish before the next starts.

    The shared action saves a fresh ratchet-baseline cache per successful push
    and later runs restore the newest match. Two overlapping pushes to `main`
    would both publish, and the older commit finishing last would leave its
    baseline as the one every pull request is then measured against. Nothing is
    cancelled: a trunk generation that has started is the one that should
    finish.
    """
    declared = workflow(PUBLISHER).get("concurrency")

    assert isinstance(declared, dict), (
        f"{PUBLISHER} must declare a concurrency group; it declares {declared!r}"
    )
    assert declared.get("cancel-in-progress") is False, (
        f"{PUBLISHER} must not cancel a running trunk generation; it declares "
        f"cancel-in-progress={declared.get('cancel-in-progress')!r}"
    )
    assert "github.ref" in str(declared.get("group")), (
        f"{PUBLISHER}'s group must serialize per ref so two pushes to main "
        f"queue; it declares {declared.get('group')!r}"
    )


def test_the_publisher_publishes_the_report() -> None:
    """Leave the publisher on the action's default.

    Setting ``publish-artefact`` here as the pull-request lane does would
    leave the upload with no report to read.
    """
    inputs = coverage_step(PUBLISHER, PUBLISHER_COVERAGE_STEP)

    assert "publish-artefact" not in inputs, (
        f"{PUBLISHER} must keep the action's default; it declares "
        f"publish-artefact={inputs.get('publish-artefact')!r}"
    )
