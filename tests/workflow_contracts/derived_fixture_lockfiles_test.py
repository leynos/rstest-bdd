"""Contract tests for the Dependabot fixture-lockfile refresh workflow.

The workflow has permission to write only because Cargo manifests changed by
Dependabot can alter the dependency resolution of the two independent
published-GPUI fixtures.  These tests constrain that authority to Dependabot
pull requests, the checked-out pull request head and the two generated
lockfiles.  They also keep the workflow limited to its lock-refresh targets;
the ordinary CI workflow continues to build and exercise the fixtures.

Run via ``make test-workflow-contracts``.
"""

from pathlib import Path

import pytest
import yaml

ROOT = Path(__file__).resolve().parents[2]
WORKFLOW_PATH = ROOT / ".github" / "workflows" / "refresh-derived-fixture-lockfiles.yml"
CI_WORKFLOW_PATH = ROOT / ".github" / "workflows" / "ci.yml"
LOCKFILES = [
    "tests/fixtures/published-gpui-0-2-2/Cargo.lock",
    "tests/fixtures/published-gpui-e2e/Cargo.lock",
]
MANIFEST_PATHS = [
    "Cargo.toml",
    "crates/**/Cargo.toml",
    "tests/fixtures/published-gpui-0-2-2/Cargo.toml",
    "tests/fixtures/published-gpui-e2e/Cargo.toml",
]


def _load(path: Path) -> dict[str, object]:
    """Parse a GitHub Actions workflow, including PyYAML's ``on`` quirk."""
    workflow = yaml.safe_load(path.read_text(encoding="utf-8"))
    assert isinstance(workflow, dict), f"{path.name} must contain a mapping"
    return workflow


def _triggers(workflow: dict[str, object]) -> dict[str, object]:
    """Return a workflow's trigger mapping."""
    triggers = workflow.get("on", workflow.get(True))
    assert isinstance(triggers, dict), "the workflow must declare an on: mapping"
    return triggers


def _job(workflow: dict[str, object]) -> dict[str, object]:
    """Return the sole lockfile-refresh job."""
    jobs = workflow.get("jobs")
    assert isinstance(jobs, dict), "the workflow must declare jobs"
    assert list(jobs) == ["refresh-lockfiles"], (
        "the workflow must declare only the Dependabot lockfile refresh job"
    )
    job = jobs["refresh-lockfiles"]
    assert isinstance(job, dict), "jobs.refresh-lockfiles must be a mapping"
    return job


def _named_step(job: dict[str, object], name: str) -> dict[str, object]:
    """Return the uniquely named step from *job*."""
    steps = job.get("steps")
    assert isinstance(steps, list), "jobs.refresh-lockfiles.steps must be a list"
    matches = [
        step for step in steps if isinstance(step, dict) and step.get("name") == name
    ]
    assert len(matches) == 1, f"expected one {name!r} step, found {len(matches)}"
    return matches[0]


def _ci_setup_rust_uses(workflow: dict[str, object]) -> str:
    """Return CI's Rust setup action reference."""
    jobs = workflow.get("jobs")
    assert isinstance(jobs, dict), "CI must declare jobs"
    build_test = jobs.get("build-test")
    assert isinstance(build_test, dict), "CI must declare jobs.build-test"
    setup = _named_step(build_test, "Setup Rust")
    uses = setup.get("uses")
    assert isinstance(uses, str), "CI's Setup Rust step must use an action"
    return uses


@pytest.fixture(scope="module")
def workflow() -> dict[str, object]:
    """Parse the lockfile-refresh workflow once for this module."""
    return _load(WORKFLOW_PATH)


@pytest.fixture(scope="module")
def refresh_job(workflow: dict[str, object]) -> dict[str, object]:
    """Return the single refresh job once for this module."""
    return _job(workflow)


def test_dependabot_pr_trigger_is_narrow_and_manifest_scoped(
    workflow: dict[str, object],
) -> None:
    """The workflow reacts only to manifest changes on lifecycle PR events."""
    triggers = _triggers(workflow)
    assert list(triggers) == ["pull_request_target"], (
        "the write-enabled workflow must have only a pull_request_target trigger"
    )
    pull_request_target = triggers["pull_request_target"]
    assert isinstance(pull_request_target, dict), (
        "pull_request_target must be a mapping"
    )
    assert pull_request_target.get("types") == ["opened", "reopened", "synchronize"], (
        "pull_request_target must run only for lifecycle pull-request events"
    )
    assert pull_request_target.get("paths") == MANIFEST_PATHS, (
        "pull_request_target must be limited to manifest changes"
    )


def test_write_permission_is_gated_to_dependabot(
    refresh_job: dict[str, object],
) -> None:
    """Only Dependabot can schedule the job that inherits write permission."""
    assert refresh_job.get("if") == "${{ github.actor == 'dependabot[bot]' }}", (
        "jobs.refresh-lockfiles must block every non-Dependabot pull request"
    )


def test_permissions_are_only_contents_write(workflow: dict[str, object]) -> None:
    """The job token has the minimum scope required to push regenerated locks."""
    assert workflow.get("permissions") == {"contents": "write"}, (
        "the workflow must have only contents write permission"
    )


def test_checkout_uses_the_dependabot_head_with_credentials(
    refresh_job: dict[str, object],
) -> None:
    """The lockfiles derive from, and push back to, the Dependabot head."""
    checkout = _named_step(refresh_job, "Checkout Dependabot pull request head")
    assert (
        checkout.get("uses")
        == "actions/checkout@3d3c42e5aac5ba805825da76410c181273ba90b1"
    ), "the checkout action must use the pinned revision"
    assert checkout.get("with") == {
        "ref": "${{ github.event.pull_request.head.sha }}",
        "persist-credentials": True,
    }, "the checkout must use the Dependabot head and retain credentials"


def test_setup_rust_matches_ci(refresh_job: dict[str, object]) -> None:
    """Lock resolution uses the same Rust setup action as normal CI."""
    setup = _named_step(refresh_job, "Setup Rust")
    assert setup.get("uses") == _ci_setup_rust_uses(_load(CI_WORKFLOW_PATH)), (
        "the lockfile refresh must use CI's Rust setup action"
    )


def test_refresh_commit_and_push_touch_only_generated_lockfiles(
    refresh_job: dict[str, object],
) -> None:
    """The job runs only refresh targets then commits and pushes their output."""
    refresh = _named_step(refresh_job, "Refresh published GPUI fixture lockfiles")
    assert refresh.get("run") == (
        "make update-published-gpui-0-2-2-lock\nmake update-published-gpui-e2e-lock\n"
    ), "the refresh step must update both published-GPUI lockfiles"

    author = _named_step(refresh_job, "Configure Git author")
    assert author.get("run") == (
        "git config user.name 'github-actions[bot]'\n"
        "git config user.email '41898282+github-actions[bot]@"
        "users.noreply.github.com'\n"
    ), "the commit author must be github-actions[bot]"

    commit = _named_step(refresh_job, "Commit refreshed lockfiles")
    commit_script = commit.get("run")
    assert isinstance(commit_script, str), "the commit step must run a script"
    for lockfile in LOCKFILES:
        assert lockfile in commit_script, f"the commit script must name {lockfile}"
    assert all(
        fragment in commit_script
        for fragment in [
            "git diff --quiet --",
            "git add --",
            "git commit -m 'chore(deps): refresh published GPUI fixture lockfiles'",
        ]
    ), "the commit script must inspect, stage, and commit the lockfile changes"

    push = _named_step(refresh_job, "Push refreshed lockfiles")
    assert push.get("if") == "${{ steps.commit.outputs.changed == 'true' }}", (
        "the push must run only after the commit step records a change"
    )
    assert (
        push.get("run")
        == "git push origin HEAD:${{ github.event.pull_request.head.ref }}"
    ), "the push must target the Dependabot pull-request head"
