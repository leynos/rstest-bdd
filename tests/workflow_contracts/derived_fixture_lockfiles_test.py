"""Contract tests for the derived fixture-lockfile workflow.

The workflow has permission to write only because Cargo manifests changed by
Dependabot can alter the dependency resolution of the standalone fixture
workspaces registered in the Makefile's ``DERIVED_LOCKFILE_MANIFESTS``.  These
tests constrain that authority to Dependabot pull requests, the checked-out
pull request head and the registered generated lockfiles.  They also keep the
workflow limited to the centralized lock-refresh target and prevent the
registry from drifting away from the lockfiles actually tracked in the
repository; the ordinary CI workflow continues to build and exercise the
fixtures and now fails early on stale locks for every contributor.

Run via ``make test-workflow-contracts``.
"""

import re
from pathlib import Path

import pytest
import yaml

ROOT = Path(__file__).resolve().parents[2]
WORKFLOW_PATH = ROOT / ".github" / "workflows" / "refresh-derived-fixture-lockfiles.yml"
CI_WORKFLOW_PATH = ROOT / ".github" / "workflows" / "ci.yml"
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


def _makefile_registry() -> list[str]:
    """Return the standalone fixture manifests registered in the Makefile.

    Returns
    -------
    list[str]
        Manifest paths exactly as ``make derived-lockfile-paths`` prints them.

    The registry lives in ``DERIVED_LOCKFILE_MANIFESTS`` so the check, the
    refresh targets and the workflow's commit surface share one list.  Parse
    the variable block so the contract holds without executing Make.
    """
    makefile = (ROOT / "Makefile").read_text(encoding="utf-8")
    block = re.search(
        r"^DERIVED_LOCKFILE_MANIFESTS :?= \\\n((?:\t.*\\\n)+)\t(\S+)\n",
        makefile,
        re.MULTILINE,
    )
    assert block, "Makefile must declare DERIVED_LOCKFILE_MANIFESTS"
    entries = [
        line.strip().rstrip("\\").strip() for line in block.group(1).splitlines()
    ]
    entries.append(block.group(2))
    manifests = [entry for entry in entries if entry]
    assert manifests, "the derived-lockfile registry must not be empty"
    return manifests


def _tracked_standalone_lockfiles() -> set[Path]:
    """Return tracked Cargo.lock files outside the root workspace lock.

    Returns
    -------
    set[Path]
        Tracked lockfiles whose manifest declares its own ``[workspace]``.

    A standalone fixture workspace is a manifest declaring its own empty
    ``[workspace]`` stanza; the root lockfile belongs to the workspace the
    repository's ``Cargo.toml`` defines and is never part of the registry.
    """
    lockfiles: set[Path] = set()
    for path in ROOT.rglob("Cargo.lock"):
        relative = path.relative_to(ROOT)
        if relative == Path("Cargo.lock"):
            continue
        top_level = relative.parts[0]
        if not path.is_file() or top_level in {"target", ".vtcode"}:
            continue
        manifest = path.with_name("Cargo.toml")
        manifest_text = manifest.read_text(encoding="utf-8")
        if re.search(r"^\[workspace\]\s*$", manifest_text, re.MULTILINE):
            lockfiles.add(relative)
    return lockfiles


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
        "pull_request_target must cover the complete pull-request lifecycle"
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
        "the workflow must grant only contents write permission"
    )


def test_checkout_uses_the_dependabot_head_with_credentials(
    refresh_job: dict[str, object],
) -> None:
    """The lockfiles derive from, and push back to, the Dependabot head."""
    checkout = _named_step(refresh_job, "Checkout Dependabot pull request head")
    assert (
        checkout.get("uses")
        == "actions/checkout@3d3c42e5aac5ba805825da76410c181273ba90b1"
    ), "checkout must use the pinned actions/checkout release"
    assert checkout.get("with") == {
        "ref": "${{ github.event.pull_request.head.sha }}",
        "persist-credentials": True,
    }, "checkout must use the pull-request head with credentials"


def test_setup_rust_matches_ci(refresh_job: dict[str, object]) -> None:
    """Lock resolution uses the same Rust setup action as normal CI."""
    setup = _named_step(refresh_job, "Setup Rust")
    assert setup.get("uses") == _ci_setup_rust_uses(_load(CI_WORKFLOW_PATH)), (
        "the refresh workflow must use the CI Rust setup action"
    )


def test_refresh_commit_and_push_touch_only_generated_lockfiles(
    refresh_job: dict[str, object],
) -> None:
    """The job runs only refresh targets then commits and pushes their output."""
    refresh = _named_step(refresh_job, "Refresh derived fixture lockfiles")
    assert refresh.get("run") == ("make update-derived-lockfiles\n"), (
        "the refresh step must run the centralized derived-lockfile target"
    )

    author = _named_step(refresh_job, "Configure Git author")
    assert author.get("run") == (
        "git config user.name 'github-actions[bot]'\n"
        "git config user.email "
        "'41898282+github-actions[bot]@users.noreply.github.com'\n"
    ), "the author step must configure the GitHub Actions bot identity"

    commit = _named_step(refresh_job, "Commit refreshed lockfiles")
    commit_script = commit.get("run")
    expected_commit_script = (
        "# `make update-derived-lockfiles` owns the registered lockfile set;\n"
        "# derive the commit surface from the same registry instead of\n"
        "# restating the list here, so the workflow cannot drift from the\n"
        "# Makefile.\n"
        'lockfiles="$(make --no-print-directory derived-lockfile-paths)"\n'
        "if git diff --quiet -- \\\n"
        "  $lockfiles; then\n"
        "  echo 'changed=false' >> \"$GITHUB_OUTPUT\"\n"
        "  exit 0\n"
        "fi\n"
        "git add -- \\\n"
        "  $lockfiles\n"
        "git commit -m 'chore(deps): refresh derived fixture lockfiles'\n"
        "echo 'changed=true' >> \"$GITHUB_OUTPUT\"\n"
    )
    assert commit_script == expected_commit_script, (
        "the commit step must stage and commit only the generated lockfiles"
    )

    push = _named_step(refresh_job, "Push refreshed lockfiles")
    assert push.get("if") == "${{ steps.commit.outputs.changed == 'true' }}", (
        "the push step must run only when lockfiles changed"
    )
    assert (
        push.get("run")
        == "git push origin HEAD:${{ github.event.pull_request.head.ref }}"
    ), "the push step must push the refreshed lockfiles to the pull-request head"


def test_registry_covers_every_tracked_standalone_lockfile() -> None:
    """The Makefile registry must list every tracked standalone fixture lock.

    A standalone Cargo workspace whose ``Cargo.lock`` is committed has to be
    validated by ``make check-derived-lockfiles``; if it is missing from
    ``DERIVED_LOCKFILE_MANIFESTS``, a dependency change elsewhere in the
    repository can stale its lockfile and only the fixture's next ``--locked``
    build would notice.
    """
    registered = {Path(manifest) for manifest in _makefile_registry()}
    registered_lockfiles = {manifest.with_name("Cargo.lock") for manifest in registered}
    assert registered_lockfiles == _tracked_standalone_lockfiles(), (
        "DERIVED_LOCKFILE_MANIFESTS must match the tracked standalone fixture"
        " workspaces exactly; register new fixtures in the Makefile registry"
    )


def test_ci_fails_early_on_stale_derived_lockfiles() -> None:
    """Normal contributor PRs must get the stale-lockfile signal from CI.

    The check runs before the fixture-specific ``--locked`` steps so the
    failure names the refresh target, and it needs no write permission: the
    Dependabot-only repair workflow keeps its restricted scope.
    """
    ci = _load(CI_WORKFLOW_PATH)
    jobs = ci.get("jobs")
    assert isinstance(jobs, dict), "CI must declare jobs"
    build_test = jobs.get("build-test")
    assert isinstance(build_test, dict), "CI must declare jobs.build-test"
    check = _named_step(build_test, "Check derived fixture lockfiles")
    assert check.get("run") == "make check-derived-lockfiles", (
        "CI must run the centralized derived-lockfile check"
    )
    assert check.get("if") == "${{ matrix.tools && runner.os == 'Linux' }}", (
        "the check must run once, on the Linux tools lane, without write access"
    )
