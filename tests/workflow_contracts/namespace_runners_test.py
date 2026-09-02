"""Verify GitHub Actions runner and workflow-ownership assignments.

Load workflow YAML and hold the runner, platform, permissions, cache, and
tooling contracts for repository-owned jobs. Reusable workflows keep their
callee-owned runner selection.

Run with:

    pytest tests/workflow_contracts/namespace_runners_test.py
"""

import re
from pathlib import Path

import yaml

ROOT = Path(__file__).resolve().parents[2]
LINUX_PROFILE = "namespace-profile-default"
WINDOWS_PROFILE = "namespace-profile-rust-windows-ci"
EXPECTED_BUILD_MATRIX = [
    {
        "os": LINUX_PROFILE,
        "coverage": True,
        "features": "",
        "with-default-features": True,
        "tools": True,
        "use-nextest": True,
    },
    {
        "os": LINUX_PROFILE,
        "coverage": True,
        "features": "strict-compile-time-validation",
        "with-default-features": False,
        "tools": True,
        "use-nextest": True,
    },
    {
        "os": WINDOWS_PROFILE,
        "coverage": True,
        "features": "",
        "with-default-features": True,
        "tools": False,
        "use-nextest": False,
    },
    {
        "os": WINDOWS_PROFILE,
        "coverage": True,
        "features": "strict-compile-time-validation",
        "with-default-features": False,
        "tools": False,
        "use-nextest": False,
    },
]


def _job(workflow_name: str, job_name: str) -> dict[str, object]:
    """Load one named job from a repository workflow."""
    workflow_path = ROOT / ".github" / "workflows" / workflow_name
    workflow = yaml.safe_load(workflow_path.read_text(encoding="utf-8"))
    assert isinstance(workflow, dict), f"{workflow_name} must parse to a mapping"
    jobs = workflow.get("jobs")
    assert isinstance(jobs, dict), f"{workflow_name} must declare jobs"
    job = jobs.get(job_name)
    assert isinstance(job, dict), f"{workflow_name} must declare {job_name}"
    return job


def _steps(job: dict[str, object]) -> list[dict[str, object]]:
    """Return a job's steps after validating their mapping shape."""
    raw_steps = job.get("steps")
    assert isinstance(raw_steps, list), "job must declare a steps list"
    steps: list[dict[str, object]] = []
    for raw_step in raw_steps:
        assert isinstance(raw_step, dict), "every workflow step must be a mapping"
        steps.append(raw_step)
    return steps


def test_comment_job_uses_the_shared_uncached_namespace_profile() -> None:
    """Keep the controlled utility-job assignment from drifting."""
    job = _job("delayed-pr-comment.yml", "delay_and_comment")
    assert job.get("runs-on") == LINUX_PROFILE, (
        f"delayed-pr-comment.yml:delay_and_comment must use {LINUX_PROFILE}"
    )
    assert job.get("timeout-minutes") == 65, (
        "delayed-pr-comment.yml:delay_and_comment must bound runner occupancy"
    )


def test_build_matrix_uses_exact_namespace_profiles() -> None:
    """Keep every feature lane on its approved native Namespace profile."""
    build_test = _job("ci.yml", "build-test")
    assert build_test.get("runs-on") == "${{ matrix.os }}", (
        "ci.yml:build-test must resolve its runner from matrix.os"
    )
    strategy = build_test.get("strategy")
    assert isinstance(strategy, dict), "ci.yml:build-test must declare strategy"
    matrix = strategy.get("matrix")
    assert isinstance(matrix, dict), "ci.yml:build-test must declare a matrix"
    include = matrix.get("include")
    assert isinstance(include, list), "ci.yml:build-test matrix must declare include"
    assert include == EXPECTED_BUILD_MATRIX, (
        "ci.yml:build-test must preserve its four feature lanes while assigning "
        f"Linux to {LINUX_PROFILE} and Windows to {WINDOWS_PROFILE}; got {include!r}"
    )


def test_build_matrix_keeps_least_privilege_and_managed_runner_auth() -> None:
    """Keep the matrix read-only without redundant Namespace authentication."""
    build_test = _job("ci.yml", "build-test")
    assert build_test.get("permissions") == {"contents": "read"}, (
        "ci.yml:build-test must retain contents: read as its only token permission"
    )
    action_uses = [str(step.get("uses", "")) for step in _steps(build_test)]
    assert not any(
        uses.startswith("namespacelabs/nscloud-setup@") for uses in action_uses
    ), "Namespace profile runners must not add redundant nscloud authentication"
    assert not any(
        uses.startswith("namespacelabs/nscloud-cache-action@") for uses in action_uses
    ), "the uncached baseline must not attach a Namespace cache volume"


def test_build_matrix_preserves_pinned_github_caches() -> None:
    """Retain the workflow's existing GitHub cache actions on uncached profiles."""
    build_test = _job("ci.yml", "build-test")
    cache_uses = [
        str(step.get("uses"))
        for step in _steps(build_test)
        if str(step.get("uses", "")).startswith("actions/cache@")
    ]
    assert len(cache_uses) == 1, (
        "ci.yml:build-test must retain the direct Merman GitHub cache; the "
        "shared Whitaker action owns its installer cache"
    )
    assert all(
        re.fullmatch(r"actions/cache@[0-9a-f]{40}", uses) for uses in cache_uses
    ), "direct GitHub cache actions must remain pinned to full commit SHAs"


def test_whitaker_uses_shared_pinned_installer_action() -> None:
    """Require the shared action that normalizes Whitaker installer paths."""
    build_test = _job("ci.yml", "build-test")
    steps = _steps(build_test)
    path_index = next(
        index
        for index, step in enumerate(steps)
        if step.get("name") == "Expose user-local tools"
    )
    install_index = next(
        index
        for index, step in enumerate(steps)
        if step.get("name") == "Install Whitaker"
    )
    path_step = steps[path_index]
    install_step = steps[install_index]
    assert path_step.get("if") == "${{ matrix.tools }}", (
        "ci.yml:user-local path setup must remain limited to Linux tools lanes"
    )
    assert path_step.get("shell") == "bash", (
        "ci.yml:user-local path setup must use the Linux runner shell"
    )
    assert path_step.get("run") == 'echo "$HOME/.local/bin" >> "$GITHUB_PATH"', (
        "ci.yml:user-local path setup must expose Whitaker dependency binaries"
    )
    assert path_index < install_index, (
        "ci.yml:user-local bin directory must be on PATH before Whitaker runs"
    )
    assert install_step.get("uses") == (
        "leynos/shared-actions/.github/actions/install-whitaker@"
        "794e4801babcf68065c660fdf4781ad62be5d061"
    ), "ci.yml:Install Whitaker must pin the shared installer action"
    assert install_step.get("with") == {
        "installer-version": "${{ env.WHITAKER_INSTALLER_VERSION }}"
    }, "ci.yml:Install Whitaker must pass the workflow's installer pin"
    assert "run" not in install_step, (
        "ci.yml:Install Whitaker must not duplicate the shared installer action"
    )


def test_windows_profile_provisions_make_before_repository_steps() -> None:
    """Install GNU Make after proving the Namespace Windows shell toolchain."""
    build_test = _job("ci.yml", "build-test")
    steps = _steps(build_test)
    setup_indices = [
        index
        for index, step in enumerate(steps)
        if step.get("name") == "Verify and install Windows runner tools"
    ]
    assert len(setup_indices) == 1, "the Windows prerequisite step must appear once"
    setup_index = setup_indices[0]
    setup = steps[setup_index]
    assert setup.get("if") == "${{ runner.os == 'Windows' }}", (
        "the Windows tool setup must not run on Linux"
    )
    assert setup.get("shell") == "pwsh", "Windows tool setup must use PowerShell"
    script = str(setup.get("run", ""))
    for required_command in (
        "Get-Command git",
        "Get-Command bash",
        "Get-Command choco",
        "choco install make --yes --no-progress",
        "Get-Command make",
    ):
        assert required_command in script, (
            f"Windows tool setup must contain {required_command!r}"
        )
    checkout_index = next(
        index for index, step in enumerate(steps) if step.get("name") == "Checkout"
    )
    assert setup_index < checkout_index, (
        "Windows Git, Bash, Chocolatey, and Make must be ready before checkout"
    )


def test_external_reusable_workflows_keep_callee_owned_runners() -> None:
    """Keep externally owned reusable jobs free to select their own runners."""
    for workflow_name, job_name in (
        ("mutation-testing.yml", "mutation"),
        ("dependabot-automerge.yml", "automerge"),
    ):
        job = _job(workflow_name, job_name)
        uses = str(job.get("uses", ""))
        assert uses.startswith("leynos/shared-actions/.github/workflows/"), (
            f"{workflow_name}:{job_name} must remain a shared reusable workflow"
        )
        assert "runs-on" not in job, (
            f"{workflow_name}:{job_name} must leave runner selection to its callee"
        )
