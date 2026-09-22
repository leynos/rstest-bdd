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


#: Runner labels this repository deploys on, mapped to the `runner.os` value
#: GitHub gives them. Named rather than inferred from the label text: a label
#: is a shape and a provider, and only a table says which operating system it
#: boots.
RUNNER_PLATFORMS = {
    "ubicloud-standard-2": "Linux",
    "windows-latest": "Windows",
}

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


def test_only_one_job_in_the_publisher_holds_the_credential() -> None:
    """Keep the CodeScene contact to one job, not merely to one workflow.

    The publisher grew a second job to write the Windows baseline. That job
    measures coverage and uploads nothing, and the guide says so; without a
    rule, giving it the credential would read as symmetry with the Linux job
    and would quietly double the number of places the token is exposed. The
    upload rules elsewhere check the job that does upload, so neither would
    notice a second one appearing.
    """
    jobs = workflow(PUBLISHER).get("jobs")
    assert isinstance(jobs, dict), f"{PUBLISHER} must declare a jobs mapping"
    holding = sorted(
        name
        for name, job in jobs.items()
        if isinstance(job, dict) and "CS_ACCESS_TOKEN" in str(job.get("env") or {})
    )

    assert holding == [PUBLISHER_JOB], (
        f"only {PUBLISHER_JOB} may hold the CodeScene credential; {holding} hold it"
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
