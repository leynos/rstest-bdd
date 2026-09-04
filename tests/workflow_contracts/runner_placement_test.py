"""Verify GitHub Actions runner placement and tooling contracts.

Linux developer-blocking work runs on Ubicloud; Windows, delayed-comment, and
scheduled work runs on GitHub-hosted runners. Reusable workflows keep their
callee-owned runner selection.

Run with:

    pytest tests/workflow_contracts/runner_placement_test.py
"""

import re

import yaml
from workflow_queries import coverage_calling_jobs as _coverage_calling_jobs
from workflow_queries import direct_workspace_test_steps as _direct_workspace_test_steps
from workflow_queries import workflow_names as _workflow_names
from workflow_support import (
    GITHUB_HOSTED_LINUX,
    GITHUB_HOSTED_WINDOWS,
    ROOT,
    UBICLOUD_LINUX_LABEL,
)
from workflow_support import (
    job as _job,
)
from workflow_support import (
    step_index as _step_index,
)
from workflow_support import (
    steps as _steps,
)

# One Linux lane on the recipe's starting shape, retained because the sampler
# measured a peak far below the smaller shape's memory. One lane, because
# `--all-features` enables
# `strict-compile-time-validation`, which only implies
# `compile-time-validation` and conflicts with no other feature.
EXPECTED_BUILD_MATRIX = [
    {
        "os": UBICLOUD_LINUX_LABEL,
        "rust-toolchain": "stable",
        "coverage": True,
        "features": "",
        "with-default-features": True,
        "tools": True,
        "use-nextest": True,
    },
    {
        "os": GITHUB_HOSTED_WINDOWS,
        "rust-toolchain": "stable-x86_64-pc-windows-msvc",
        "coverage": True,
        "features": "",
        "with-default-features": True,
        "tools": False,
        "use-nextest": False,
    },
    {
        "os": GITHUB_HOSTED_WINDOWS,
        "rust-toolchain": "stable-x86_64-pc-windows-msvc",
        "coverage": True,
        "features": "strict-compile-time-validation",
        "with-default-features": False,
        "tools": False,
        "use-nextest": False,
    },
]


def test_comment_job_stays_on_github_hosted_linux() -> None:
    """Keep API-bound delay work off paid Linux compute."""
    job = _job("delayed-pr-comment.yml", "delay_and_comment")
    assert job.get("runs-on") == GITHUB_HOSTED_LINUX, (
        f"delayed-pr-comment.yml:delay_and_comment must use {GITHUB_HOSTED_LINUX}"
    )
    assert job.get("timeout-minutes") == 65, (
        "delayed-pr-comment.yml:delay_and_comment must bound runner occupancy"
    )


def test_build_matrix_uses_exact_runner_labels() -> None:
    """Keep Linux lanes on Ubicloud and Windows lanes on GitHub-hosted."""
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
        "ci.yml:build-test must keep one Linux lane and two Windows lanes while "
        f"Linux to {UBICLOUD_LINUX_LABEL} and Windows to {GITHUB_HOSTED_WINDOWS}; "
        f"got {include!r}"
    )


def test_one_job_executes_the_workspace_suite() -> None:
    """No job may duplicate the executed set of the coverage lane."""
    coverage_callers = _coverage_calling_jobs()
    direct_runners = _direct_workspace_test_steps()
    assert coverage_callers == ["ci.yml:build-test"], (
        "only ci.yml:build-test may run the coverage driver; a second job with "
        f"the same platform and features would execute nothing new, got "
        f"{coverage_callers}"
    )
    assert not direct_runners, (
        "the coverage action is the only workspace test execution, doc tests "
        "included, so no step may invoke cargo test, cargo nextest, or make "
        f"test directly; found {direct_runners}"
    )


def test_linux_coverage_is_the_only_linux_test_execution() -> None:
    """One instrumented run covers the whole workspace and every feature."""
    steps = _steps(_job("ci.yml", "build-test"))
    generator = steps[_step_index(steps, "Test and Measure Coverage (Linux)")]
    assert generator.get("if") == "${{ runner.os == 'Linux' }}", (
        "the Linux coverage step must own every Linux lane"
    )
    assert "continue-on-error" not in generator, (
        "the sole Linux test execution must fail the build"
    )
    inputs = generator.get("with")
    assert isinstance(inputs, dict), "the coverage step must declare inputs"
    assert inputs.get("all-features") == "true", (
        "the single execution must enable every feature, which is what makes "
        "a separate strict-compile-time-validation lane redundant"
    )
    assert inputs.get("all-targets") == "true", (
        "the single execution must build every target"
    )
    assert inputs.get("use-cargo-nextest") == "true", (
        "the Linux lane runs nextest under llvm-cov"
    )
    assert inputs.get("with-ratchet") == "true", (
        "the sole Linux lane owns the ratchet baseline"
    )
    assert inputs.get("doctests") == "true", (
        "llvm-cov's nextest path cannot execute doc tests, so the action must "
        "run them after the instrumented run with the same feature selection"
    )
    assert "features" not in inputs, (
        "an explicit feature list would contradict all-features"
    )
    direct_runners = _direct_workspace_test_steps()
    assert not direct_runners, (
        "the coverage action is the only workspace test execution, doc tests "
        f"included, so no bespoke test step may sit beside it; found "
        f"{direct_runners}"
    )


def test_shared_cache_keys_name_the_pinned_default_toolchain() -> None:
    """The Linux lanes request the toolchain that rust-toolchain.toml pins."""
    toolchain = (ROOT / "rust-toolchain.toml").read_text(encoding="utf-8")
    assert 'channel = "stable"' in toolchain, (
        "the Linux lanes request 'stable' explicitly; the cache keys and the "
        "pinned channel must not disagree"
    )


def test_registered_self_hosted_labels_match_the_matrix() -> None:
    """Actionlint must know every intentional self-hosted label, and no more."""
    configuration = yaml.safe_load(
        (ROOT / ".github" / "actionlint.yaml").read_text(encoding="utf-8")
    )
    assert isinstance(configuration, dict), "actionlint.yaml must parse to a mapping"
    self_hosted = configuration.get("self-hosted-runner")
    assert isinstance(self_hosted, dict), "actionlint.yaml must declare runner labels"
    assert self_hosted.get("labels") == [UBICLOUD_LINUX_LABEL], (
        "actionlint.yaml must register exactly the Ubicloud labels the "
        "repository uses; GitHub-hosted labels need no registration"
    )


def test_build_matrix_keeps_least_privilege_and_no_retired_provider() -> None:
    """Keep the matrix read-only and free of the retired Namespace wiring."""
    build_test = _job("ci.yml", "build-test")
    assert build_test.get("permissions") == {"contents": "read"}, (
        "ci.yml:build-test must retain contents: read as its only token permission"
    )
    action_uses = [str(step.get("uses", "")) for step in _steps(build_test)]
    for retired_prefix in ("namespacelabs/nscloud-setup@", "namespacelabs/"):
        assert not any(uses.startswith(retired_prefix) for uses in action_uses), (
            f"ci.yml:build-test must not reference {retired_prefix}"
        )


def test_build_matrix_installs_merman_without_a_source_build() -> None:
    """Require the pinned prebuilt Merman installation path."""
    steps = _steps(_job("ci.yml", "build-test"))
    merman_step = steps[_step_index(steps, "Install prebuilt Merman CLI")]
    install_command = str(merman_step.get("run"))
    required_fragments = {
        "dfdc2a978a884aa5a2ad5b85285fb5175cb435e82cf96efa860a550749e09d99",
        "sha256sum --check",
        '"$tool_dir/merman-cli" --version',
        "https://github.com/Latias94/merman/releases/download/",
    }
    missing_fragments = sorted(
        fragment for fragment in required_fragments if fragment not in install_command
    )
    assert not missing_fragments, (
        f"ci.yml:Merman installer omits required fragments: {missing_fragments}"
    )
    assert "cargo binstall" not in install_command, (
        "ci.yml:Merman installer must not fall back to a Cargo source build"
    )


def test_whitaker_uses_shared_pinned_installer_action() -> None:
    """Require the shared action, with the caller owning its cached paths."""
    build_test = _job("ci.yml", "build-test")
    steps = _steps(build_test)
    install_step = steps[_step_index(steps, "Install Whitaker")]
    assert install_step.get("if") == "${{ matrix.tools }}", (
        "ci.yml:Install Whitaker must remain limited to Linux tools lanes"
    )
    assert re.fullmatch(
        r"leynos/shared-actions/\.github/actions/install-whitaker@[0-9a-f]{40}",
        str(install_step.get("uses", "")),
    ), "ci.yml:Install Whitaker must pin the shared installer action"
    assert install_step.get("with") == {
        "installer-version": "${{ env.WHITAKER_INSTALLER_VERSION }}",
        "cache-provider": "external",
    }, "ci.yml:Install Whitaker must pass the installer pin and cede caching"
    assert "run" not in install_step, (
        "ci.yml:Install Whitaker must not duplicate the shared installer action"
    )
    assert not any(step.get("name") == "Expose user-local tools" for step in steps), (
        "ci.yml must not shadow Cargo-installed tools with an XDG PATH override"
    )


def test_windows_lane_installs_only_the_missing_prerequisite() -> None:
    """GitHub-hosted Windows ships Git, Bash, and Chocolatey, but not Make."""
    steps = _steps(_job("ci.yml", "build-test"))
    setup_index = _step_index(steps, "Install GNU Make")
    setup = steps[setup_index]
    assert setup.get("if") == "${{ runner.os == 'Windows' }}", (
        "the GNU Make prerequisite must not run on Linux"
    )
    assert setup.get("shell") == "pwsh", "the Windows prerequisite must use PowerShell"
    script = str(setup.get("run", ""))
    assert "choco install make --yes --no-progress" in script, (
        "`make publish-check` runs on every lane, so Windows must install Make"
    )
    for redundant_probe in ("Get-Command git", "Get-Command bash", "Get-Command choco"):
        assert redundant_probe not in script, (
            f"the GitHub-hosted Windows image already provides {redundant_probe!r}"
        )
    assert setup_index < _step_index(steps, "Checkout"), (
        "GNU Make must be ready before repository code runs"
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


def test_scheduled_and_administrative_jobs_stay_github_hosted() -> None:
    """Non-blocking work must not consume paid Ubicloud capacity."""
    for workflow_name, job_name in (
        ("refresh-derived-fixture-lockfiles.yml", "refresh-lockfiles"),
        ("delayed-pr-comment.yml", "delay_and_comment"),
    ):
        job = _job(workflow_name, job_name)
        assert job.get("runs-on") == GITHUB_HOSTED_LINUX, (
            f"{workflow_name}:{job_name} must stay on {GITHUB_HOSTED_LINUX}"
        )


def test_workflows_never_fall_back_to_a_source_build() -> None:
    """Fail closed when a trusted prebuilt binary is unavailable."""
    workflow_directory = ROOT / ".github" / "workflows"
    for name in _workflow_names():
        workflow_path = workflow_directory / name
        text = workflow_path.read_text(encoding="utf-8")
        assert "cargo install" not in text, (
            f"{workflow_path.name} must not compile a tool from source; install "
            "a checksum-verified or signed prebuilt binary instead"
        )
        assert "taiki-e/install-action" not in text, (
            f"{workflow_path.name} must not use taiki-e/install-action without a "
            "reviewed 'fallback: none' exception"
        )
        assert "nscloud-cache-action" not in text, (
            f"{workflow_path.name} must not retain the retired Namespace cache "
            "action; Ubicloud runners have no persistent volume"
        )
        if "cargo binstall" in text:
            assert "--disable-strategies compile" in text, (
                f"{workflow_path.name} must disable Binstall's compile strategy "
                "so a missing binary fails the job"
            )
            assert "--only-signed" in text, (
                f"{workflow_path.name} must require a signed Binstall artefact"
            )
