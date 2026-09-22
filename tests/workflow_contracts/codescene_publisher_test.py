"""CV-005: the shape of the workflow that owns CodeScene publication.

`codescene_coverage_test` clears the lanes a pull request can reach. These
rules are the other half: what the one workflow allowed to hold the credential
must look like. Its triggers, the mode it uploads in, the ref it publishes
from, and whether two trunk generations can overlap are each a way the estate
rule fails while every pull-request lane stays clean.

Run via ``make test-workflow-contracts``.
"""

import re

from codescene_coverage_support import (
    PR_WORKFLOW,
    PUBLISHER,
    PUBLISHER_COVERAGE_STEP,
    PUBLISHER_JOB,
    coverage_step,
)
from coverage_lane_pairs import RUNNER_PLATFORMS
from workflow_queries import iter_steps, workflow_names
from workflow_support import ROOT, workflow


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
    inputs = coverage_step(PUBLISHER, PUBLISHER_COVERAGE_STEP, PUBLISHER_JOB)

    assert "publish-artefact" not in inputs, (
        f"{PUBLISHER} must keep the action's default; it declares "
        f"publish-artefact={inputs.get('publish-artefact')!r}"
    )


def test_no_publisher_job_widens_when_the_baseline_is_written() -> None:
    """Leave a dispatch from any ref unable to move a ratchet baseline.

    The shared action's `publish-baseline` default, `auto`, saves only on a
    push to `refs/heads/main`, which is what lets this workflow's dispatch
    trigger measure a branch without advancing the baseline every pull
    request is compared against. `always` would hand that decision to this
    workflow, whose triggers do not make it.
    """
    widened = {
        reference.job: inputs.get("publish-baseline")
        for reference in iter_steps(PUBLISHER)
        if "generate-coverage@" in reference.uses
        and isinstance(inputs := reference.step.get("with"), dict)
        and inputs.get("publish-baseline", "auto") != "auto"
    }

    assert not widened, (
        f"every {PUBLISHER} job must keep publish-baseline at auto; {widened}"
    )


_PLATFORM_GUARD = re.compile(r"runner\.os == '(?P<platform>\w+)'")


def _gate_ratcheting_platforms() -> set[str]:
    """Return the platforms the merge gate ratchets on.

    The gate resolves its runner from a matrix, so the platform is read from
    each step's own guard rather than from `runs-on`.

    Returns
    -------
    set[str]
        Each `runner.os` value a ratcheting coverage step is guarded to.
    """
    found = set()
    for reference in iter_steps(PR_WORKFLOW):
        inputs = reference.step.get("with")
        if "generate-coverage@" not in reference.uses:
            continue
        if not isinstance(inputs, dict) or inputs.get("with-ratchet") != "true":
            continue
        found.update(_PLATFORM_GUARD.findall(str(reference.step.get("if", ""))))
    return found


def _publisher_ratcheting_platforms() -> set[str]:
    """Return the platforms the trunk writes a baseline on.

    Returns
    -------
    set[str]
        Each `runner.os` value a ratcheting publisher job runs on.
    """
    jobs = workflow(PUBLISHER).get("jobs")
    assert isinstance(jobs, dict), f"{PUBLISHER} must declare a jobs mapping"
    found = set()
    for name, job in jobs.items():
        if not isinstance(job, dict):
            continue
        ratchets = any(
            "generate-coverage@" in str(step.get("uses", ""))
            and isinstance(step.get("with"), dict)
            and step["with"].get("with-ratchet") == "true"
            for step in job.get("steps") or []
            if isinstance(step, dict)
        )
        if not ratchets:
            continue
        label = str(job.get("runs-on"))
        assert label in RUNNER_PLATFORMS, (
            f"{PUBLISHER}:{name} runs on {label!r}, which the platform table "
            f"does not name; it knows {sorted(RUNNER_PLATFORMS)}"
        )
        found.add(RUNNER_PLATFORMS[label])
    return found


def test_every_ratcheting_platform_has_a_trunk_writer() -> None:
    """Give every ratcheting lane a baseline to read.

    `generate-coverage` keys the baseline cache by `runner.os` alone. A lane
    that ratchets on a platform the trunk never runs therefore restores
    nothing, and the action writes a zero in its place: the lane reports a
    ratchet, compares against zero, and passes whatever its coverage is. That
    is the failure this rule exists to refuse, and it is invisible from the
    lane itself, which looks configured correctly and goes green.

    Asserted as set equality, so the converse fails too: a trunk writer for a
    platform nothing reads is a job paying for a baseline no lane consults.
    """
    gate = _gate_ratcheting_platforms()
    publisher = _publisher_ratcheting_platforms()

    assert gate, f"{PR_WORKFLOW} must ratchet on at least one platform"
    assert gate == publisher, (
        f"every platform the gate ratchets on needs a trunk baseline writer "
        f"and no others; the gate ratchets on {sorted(gate)} and the trunk "
        f"writes {sorted(publisher)}"
    )


def test_each_publisher_job_pins_the_toolchain_its_gate_leg_uses() -> None:
    """Restore the Cargo generation the merge gate published, not another.

    Every publisher job is a cache *reader*: the merge gate's trunk run writes
    the registry and tool archives, and the key carries the toolchain. A
    publisher job naming a different toolchain therefore restores nothing and
    compiles the workspace from cold on every trunk run, which is slow and
    entirely invisible, because a cold run is a correct run.

    The gate takes its toolchain from a matrix row keyed by the runner label,
    so the two are matched on that label rather than on platform: two legs can
    share `runner.os` and differ in toolchain.
    """
    gate_jobs = workflow(PR_WORKFLOW).get("jobs")
    assert isinstance(gate_jobs, dict), f"{PR_WORKFLOW} must declare jobs"
    gate = gate_jobs.get("build-test")
    assert isinstance(gate, dict), f"{PR_WORKFLOW} must declare build-test"
    strategy = gate.get("strategy")
    assert isinstance(strategy, dict), "build-test must declare a strategy"
    matrix = strategy.get("matrix")
    assert isinstance(matrix, dict), "build-test must declare a matrix"
    include = matrix.get("include")
    assert isinstance(include, list), "the matrix must declare include rows"
    by_label = {
        str(row["os"]): str(row["rust-toolchain"])
        for row in include
        if isinstance(row, dict)
    }
    jobs = workflow(PUBLISHER).get("jobs")
    assert isinstance(jobs, dict), f"{PUBLISHER} must declare a jobs mapping"

    checked = []
    for name, job in jobs.items():
        if not isinstance(job, dict):
            continue
        label = str(job.get("runs-on"))
        declared = str((job.get("env") or {}).get("RUST_TOOLCHAIN"))
        assert label in by_label, (
            f"{PUBLISHER}:{name} runs on {label!r}, which the gate's matrix "
            f"does not use; it uses {sorted(by_label)}"
        )
        checked.append(name)
        assert declared == by_label[label], (
            f"{PUBLISHER}:{name} pins {declared!r} while the gate leg on "
            f"{label!r} uses {by_label[label]!r}; the publisher would restore "
            "a different Cargo generation and compile cold every run"
        )

    assert checked, f"{PUBLISHER} must declare at least one job"


#: Uploader revisions verified to carry the committed ``cli-manifest.json``
#: that pins ``cs-coverage``. An allowlist rather than "any full SHA": a
#: different revision is a different manifest, or none, whatever the shape of
#: its identifier, and an older one installs the floating CLI that could not
#: parse its own cobertura output. A newer verified pin is added here; the
#: rule is a floor on what is trusted, not a request to downgrade.
MANIFEST_PINNED_UPLOADERS = frozenset({"a5765019912a8ab6882b12db049c7cde635f3a85"})
UPLOADER_ACTION = "leynos/shared-actions/.github/actions/upload-codescene-coverage"
#: The dispatch workflow whose only output was the retired digest variable.
DIGEST_REFRESH_WORKFLOW = "get-codescene-sha.yml"


def test_every_uploader_call_is_on_a_manifest_pinned_revision() -> None:
    """Hold every CodeScene upload to a revision carrying the CLI manifest.

    `test_every_shared_coverage_action_is_sha_pinned` is satisfied by any full
    SHA, including the revisions whose unpinned CLI broke every pull request
    here between 2026-09-16 and 2026-09-18. The reference set is asserted
    non-empty first, so deleting the calls cannot satisfy the rule.
    """
    revisions = {
        reference.uses.partition("@")[2]
        for name in workflow_names()
        for reference in iter_steps(name)
        if reference.uses.partition("@")[0] == UPLOADER_ACTION
    }

    assert revisions, f"this repository must call {UPLOADER_ACTION}"
    assert revisions <= MANIFEST_PINNED_UPLOADERS, (
        f"every upload must use a revision verified to carry the CLI "
        f"manifest; {sorted(revisions - MANIFEST_PINNED_UPLOADERS)} is not one"
    )


def test_the_digest_refresh_workflow_is_absent() -> None:
    """Keep the dispatch that maintained the retired variable out of the tree.

    This repository never carried it. The rule is estate-wide, and a copy
    that names no variable at all would pass the text rule refusing the
    variable while still being a workflow that maintains nothing.
    """
    path = ROOT / ".github" / "workflows" / DIGEST_REFRESH_WORKFLOW

    assert not path.exists(), (
        f"{DIGEST_REFRESH_WORKFLOW} refreshed a variable nothing reads; it must "
        "not exist"
    )
