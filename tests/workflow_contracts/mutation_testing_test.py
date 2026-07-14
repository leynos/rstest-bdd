"""Contract tests for the mutation-testing caller workflow.

The executable logic lives in the ``leynos/shared-actions`` reusable
workflow, which carries its own unit and integration tests; rstest-bdd's
caller is declarative configuration. These tests parse the caller with
PyYAML and assert the contract it must uphold: the caller references the
correct reusable workflow at a commit SHA (Dependabot owns the SHA value,
so drift in the pinned commit is not a contract violation), and the
caller keeps its permissions, triggers, and ``with`` inputs. Drift such as
repointing the pin at a branch, widening permissions, or losing the
workspace paths, fixture excludes, or feature arguments fails CI on the
pull request rather than surfacing in a scheduled or manual run.

Run via ``make test-workflow-contracts``.
"""

from __future__ import annotations

import re
from pathlib import Path

import pytest
import yaml

WORKFLOW_PATH = (
    Path(__file__).resolve().parents[2] / ".github" / "workflows" / "mutation-testing.yml"
)

#: The reusable workflow path must be pinned to a full 40-hex commit SHA
#: (not a branch or tag). Dependabot owns the SHA value; this contract
#: only asserts the shape of the pin.
USES_RE = re.compile(
    r"^leynos/shared-actions/\.github/workflows/mutation-cargo\.yml@[0-9a-f]{40}$"
)

#: The exact caller configuration: workspace source under crates/;
#: example applications, test-fixture crates, and test-support modules
#: excluded as noise; feature-gated tests enabled to match `make test`.
EXPECTED_WITH = {
    "paths": "crates/",
    "exclude-globs": (
        "examples/**,"
        "crates/cargo-bdd/tests/fixtures/**,"
        "crates/*/tests/fixtures_macros/**,"
        "crates/rstest-bdd/tests/ui_lints/**,"
        "crates/rstest-bdd/tests/ui_macros/**,"
        "crates/rstest-bdd/src/test_support.rs,"
        "crates/rstest-bdd-server/src/test_support.rs,"
        "crates/rstest-bdd-harness/src/binary_test_support.rs,"
        "crates/rstest-bdd-harness/src/macrotest_support.rs,"
        "crates/rstest-bdd-harness/src/test_utils.rs,"
        "crates/rstest-bdd-harness/src/trybuild_staging/**"
    ),
    "extra-args": "--all-features --test-workspace=true",
}


def _load() -> dict[str, object]:
    """Parse the workflow file."""
    return yaml.safe_load(WORKFLOW_PATH.read_text(encoding="utf-8"))


def _triggers(workflow: dict[str, object]) -> dict[str, object]:
    """Return the ``on:`` mapping (PyYAML parses the bare key as True)."""
    triggers = workflow.get("on", workflow.get(True))
    assert isinstance(triggers, dict), "the workflow must declare an on: mapping"
    return triggers


def _mutation_job(workflow: dict[str, object]) -> dict[str, object]:
    """Return the single calling job."""
    jobs = workflow.get("jobs")
    assert isinstance(jobs, dict), "the workflow must declare a jobs mapping"
    assert jobs, "the workflow must declare at least one job"
    assert list(jobs) == ["mutation"], (
        f"expected a single job named 'mutation', found {sorted(jobs)}"
    )
    return jobs["mutation"]


@pytest.fixture(scope="module")
def workflow() -> dict[str, object]:
    """Parse the workflow file once for the module."""
    return _load()


@pytest.fixture(scope="module")
def mutation_job(workflow: dict[str, object]) -> dict[str, object]:
    """Return the single calling job."""
    return _mutation_job(workflow)


def test_uses_reference_is_pinned_to_a_commit_sha(
    mutation_job: dict[str, object],
) -> None:
    """The job must call the shared workflow pinned to a commit SHA.

    Dependabot owns the SHA value, so this asserts the shape of the pin
    (correct reusable-workflow path, full 40-hex commit SHA) rather than
    a specific commit.
    """
    uses = mutation_job.get("uses")
    assert uses is not None, "jobs.mutation.uses is missing"
    assert USES_RE.match(uses), (
        f"jobs.mutation.uses must reference mutation-cargo.yml pinned to a "
        f"full 40-character lowercase-hex commit SHA, not a branch or tag: "
        f"{uses!r}"
    )


def test_job_permissions_are_exactly_least_privilege(
    mutation_job: dict[str, object],
) -> None:
    """The job grants contents: read and id-token: write, nothing broader."""
    permissions = mutation_job.get("permissions")
    assert permissions == {"contents": "read", "id-token": "write"}, (
        "jobs.mutation.permissions must be exactly "
        f"{{'contents': 'read', 'id-token': 'write'}}, got {permissions!r}"
    )


def test_workflow_default_permissions_are_empty(
    workflow: dict[str, object],
) -> None:
    """The workflow-level default token scope is empty."""
    assert workflow.get("permissions") == {}, (
        f"top-level permissions must be an empty mapping, got "
        f"{workflow.get('permissions')!r}"
    )


def test_concurrency_serializes_per_ref_without_cancelling(
    workflow: dict[str, object],
) -> None:
    """Runs queue per ref instead of cancelling one another."""
    concurrency = workflow.get("concurrency")
    assert isinstance(concurrency, dict), "the workflow must declare concurrency"
    assert concurrency.get("group") == "mutation-testing-${{ github.ref }}", (
        f"concurrency.group must key on the triggering ref, got "
        f"{concurrency.get('group')!r}"
    )
    assert concurrency.get("cancel-in-progress") is False, (
        f"concurrency.cancel-in-progress must be false, got "
        f"{concurrency.get('cancel-in-progress')!r}"
    )


def test_triggers_keep_schedule_and_plain_dispatch(
    workflow: dict[str, object],
) -> None:
    """The daily schedule stays; dispatch has no legacy branch input."""
    triggers = _triggers(workflow)
    schedule = triggers.get("schedule")
    assert schedule == [{"cron": "35 3 * * *"}], (
        f"on.schedule must be the daily 03:35 UTC cron, got {schedule!r}"
    )
    assert "workflow_dispatch" in triggers, "on.workflow_dispatch is missing"
    dispatch = triggers.get("workflow_dispatch") or {}
    inputs = dispatch.get("inputs") or {}
    assert "branch" not in inputs, (
        "on.workflow_dispatch must not declare a branch input; the Actions "
        "run-workflow control selects the ref"
    )


def test_with_block_carries_the_caller_configuration(
    mutation_job: dict[str, object],
) -> None:
    """The caller passes exactly the paths, excludes, and feature args."""
    with_block = mutation_job.get("with")
    assert isinstance(with_block, dict), "jobs.mutation.with is missing"
    assert with_block == EXPECTED_WITH, (
        "jobs.mutation.with must configure exactly the workspace paths, the "
        "example/fixture/test-support excludes, and --all-features; got "
        f"{with_block!r}, expected {EXPECTED_WITH!r}"
    )
