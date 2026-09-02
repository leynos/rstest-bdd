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
DELAYED_COMMENT_RUNNER = "ubuntu-latest"
LINUX_PROFILE = "namespace-profile-rust-linux-ci"
WINDOWS_PROFILE = "namespace-profile-rust-windows-ci"
NAMESPACE_CACHE_ACTION = (
    "namespacelabs/nscloud-cache-action@c5f8dab7560444c4bf8dbc64f1b203431873c547"
)
SHARED_SETUP_RUST_CACHE_PROVIDER_HEAD = "5daae0a332441d170d88ca648c9e71f0bbe96cb3"
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


def test_comment_job_stays_on_github_hosted_linux() -> None:
    """Keep API-bound delay work off paid Namespace compute."""
    job = _job("delayed-pr-comment.yml", "delay_and_comment")
    assert job.get("runs-on") == DELAYED_COMMENT_RUNNER, (
        f"delayed-pr-comment.yml:delay_and_comment must use {DELAYED_COMMENT_RUNNER}"
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
    assert NAMESPACE_CACHE_ACTION in action_uses, (
        "ci.yml:build-test must mount the approved Namespace cache action"
    )


def test_build_matrix_uses_one_cache_owner_for_workflow_paths() -> None:
    """Keep direct cache paths on the Namespace volume, not GitHub archives."""
    build_test = _job("ci.yml", "build-test")
    steps = _steps(build_test)
    cache_step = next(
        step for step in steps if step.get("uses") == NAMESPACE_CACHE_ACTION
    )
    assert cache_step.get("id") == "namespace-cache", (
        "ci.yml:Namespace cache action must expose its cache-hit output"
    )
    assert cache_step.get("with") == {
        "path": (
            "~/.bun/install/cache\n"
            "~/.cargo/bin\n"
            "~/.cargo/git\n"
            "~/.cargo/registry\n"
            "~/.cache/sccache\n"
            "~/.cache/uv\n"
            "~/.local/share/uv\n"
            "${{ github.workspace }}/.ci-tools\n"
        ),
    }, "ci.yml:Namespace cache action must own the selected cache paths"
    assert not any(
        str(step.get("uses", "")).startswith("actions/cache@") for step in steps
    ), "ci.yml:build-test must not overlap Namespace-managed paths with GitHub caches"
    cache_index = steps.index(cache_step)
    checkout_index = next(
        index for index, step in enumerate(steps) if step.get("name") == "Checkout"
    )
    assert checkout_index < cache_index, (
        "ci.yml:Namespace cache wiring must happen after checkout"
    )
    setup_index = next(
        index for index, step in enumerate(steps) if step.get("name") == "Setup Rust"
    )
    assert cache_index < setup_index, (
        "ci.yml:Namespace cache wiring must precede toolchain installation"
    )


def test_build_matrix_configures_external_cache_and_bounded_parallelism() -> None:
    """Keep cache ownership and build concurrency within the runner capacity."""
    build_test = _job("ci.yml", "build-test")
    env = build_test.get("env")
    assert isinstance(env, dict), "ci.yml:build-test must declare an environment"
    assert env.get("CARGO_BUILD_JOBS") == "4", (
        "ci.yml:build-test must cap Cargo builds at the four-vCPU runner limit"
    )
    assert env.get("NEXTEST_TEST_THREADS") == "4", (
        "ci.yml:build-test must cap nextest at the four-vCPU runner limit"
    )
    setup_step = next(
        step for step in _steps(build_test) if step.get("name") == "Setup Rust"
    )
    assert setup_step.get("uses") == (
        "leynos/shared-actions/.github/actions/setup-rust@"
        f"{SHARED_SETUP_RUST_CACHE_PROVIDER_HEAD}"
    ), "ci.yml:Setup Rust must use the temporary external-cache provider revision"
    assert setup_step.get("with") == {
        "cache-provider": "external",
        "use-sccache": "false",
    }, "ci.yml:Setup Rust must leave cache ownership to Namespace"


def test_build_matrix_reports_cache_and_sccache_state() -> None:
    """Expose cache outcomes without enabling the shared sccache cache backend."""
    steps = _steps(_job("ci.yml", "build-test"))
    cache_summary = next(
        step for step in steps if step.get("name") == "Report Namespace cache status"
    )
    assert "steps.namespace-cache.outputs.cache-hit" in str(cache_summary.get("env")), (
        "ci.yml:cache summary must report the Namespace cache-hit output"
    )
    assert "GITHUB_STEP_SUMMARY" in str(cache_summary.get("run")), (
        "ci.yml:cache summary must write to the job summary"
    )
    sccache_install = next(
        step for step in steps if step.get("name") == "Install prebuilt sccache"
    )
    sccache_install_command = str(sccache_install.get("run"))
    required_fragments = {
        "cargo binstall",
        "--disable-strategies compile",
        "--only-signed",
        "RUSTC_WRAPPER",
    }
    missing_fragments = sorted(
        fragment
        for fragment in required_fragments
        if fragment not in sccache_install_command
    )
    assert not missing_fragments, (
        f"ci.yml:sccache installer omits required fragments: {missing_fragments}"
    )
    sccache_summary = next(
        step for step in steps if step.get("name") == "Report sccache statistics"
    )
    assert "sccache --show-stats" in str(sccache_summary.get("run")), (
        "ci.yml must expose sccache statistics when the binary is available"
    )


def test_build_matrix_installs_merman_without_a_source_build() -> None:
    """Require the pinned prebuilt Merman installation path."""
    steps = _steps(_job("ci.yml", "build-test"))
    merman_step = next(
        step for step in steps if step.get("name") == "Install prebuilt Merman CLI"
    )
    install_command = str(merman_step.get("run"))
    required_fragments = {
        "cargo binstall",
        "--disable-strategies compile",
        "--only-signed",
        '--install-path "$tool_dir"',
    }
    missing_fragments = sorted(
        fragment for fragment in required_fragments if fragment not in install_command
    )
    assert not missing_fragments, (
        f"ci.yml:Merman installer omits required fragments: {missing_fragments}"
    )


def test_whitaker_uses_shared_pinned_installer_action() -> None:
    """Require the shared action that normalizes Whitaker installer paths."""
    build_test = _job("ci.yml", "build-test")
    steps = _steps(build_test)
    install_step = next(
        step for step in steps if step.get("name") == "Install Whitaker"
    )
    assert install_step.get("if") == "${{ matrix.tools }}", (
        "ci.yml:Install Whitaker must remain limited to Linux tools lanes"
    )
    assert re.fullmatch(
        r"leynos/shared-actions/\.github/actions/install-whitaker@[0-9a-f]{40}",
        str(install_step.get("uses", "")),
    ), "ci.yml:Install Whitaker must pin the shared installer action"
    assert install_step.get("with") == {
        "installer-version": "${{ env.WHITAKER_INSTALLER_VERSION }}"
    }, "ci.yml:Install Whitaker must pass the workflow's installer pin"
    assert "run" not in install_step, (
        "ci.yml:Install Whitaker must not duplicate the shared installer action"
    )
    assert not any(step.get("name") == "Expose user-local tools" for step in steps), (
        "ci.yml must not shadow Cargo-installed tools with an XDG PATH override"
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
