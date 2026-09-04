"""Verify the runner-shape contracts: occupancy, parallelism, and memory.

The Linux lane sits on the recipe's ceiling shape provisionally, so the
measurement that would justify returning to the smaller one has to exist.

Run with:

    pytest tests/workflow_contracts/runner_resources_test.py
"""

from workflow_support import (
    GITHUB_HOSTED_WINDOWS,
    GITHUB_WINDOWS_VCPUS,
    SCCACHE_DIRECTORY,
    UBICLOUD_LINUX_LABEL,
    UBICLOUD_LINUX_VCPUS,
)
from workflow_support import job as _job
from workflow_support import step_index as _step_index
from workflow_support import steps as _steps


def test_ubicloud_jobs_bound_their_runner_occupancy() -> None:
    """Just-in-time self-hosted runners have no six-hour hosted limit."""
    for workflow_name, job_name in (("ci.yml", "build-test"),):
        job = _job(workflow_name, job_name)
        timeout = job.get("timeout-minutes")
        assert isinstance(timeout, int), (
            f"{workflow_name}:{job_name} must declare timeout-minutes"
        )
        assert timeout > 0, (
            f"{workflow_name}:{job_name} must declare a positive timeout-minutes"
        )


def test_build_matrix_derives_parallelism_from_named_vcpu_constants() -> None:
    """Keep build concurrency within each runner shape's vCPU count."""
    build_test = _job("ci.yml", "build-test")
    env = build_test.get("env")
    assert isinstance(env, dict), "ci.yml:build-test must declare an environment"
    assert env.get("UBICLOUD_LINUX_VCPUS") == UBICLOUD_LINUX_VCPUS, (
        f"ci.yml:build-test must name {UBICLOUD_LINUX_LABEL}'s vCPU count"
    )
    assert env.get("GITHUB_WINDOWS_VCPUS") == GITHUB_WINDOWS_VCPUS, (
        f"ci.yml:build-test must name {GITHUB_HOSTED_WINDOWS}'s vCPU count"
    )
    assert env.get("SCCACHE_DIR") == SCCACHE_DIRECTORY, (
        "ci.yml:build-test must point sccache at one explicit workspace "
        "directory so the same path is cacheable on both platforms"
    )
    assert env.get("SCCACHE_CACHE_SIZE") == "4G", (
        "ci.yml:build-test must size the compiler cache for both build shapes, "
        "because no job archives a target tree and sccache owns every "
        "compiler output"
    )
    steps = _steps(build_test)
    parallelism = steps[
        _step_index(steps, "Configure runner parallelism and compiler cache")
    ]
    script = str(parallelism.get("run", ""))
    for required_fragment in (
        "CARGO_BUILD_JOBS=%s",
        "NEXTEST_TEST_THREADS=%s",
        "SCCACHE_GHA_ENABLED=%s",
    ):
        assert required_fragment in script, (
            f"the parallelism step must export {required_fragment!r}"
        )
    step_env = parallelism.get("env")
    assert isinstance(step_env, dict), "the parallelism step must declare env"
    assert step_env.get("LINUX_VCPUS") == "${{ env.UBICLOUD_LINUX_VCPUS }}", (
        "the parallelism step must read the named Linux vCPU constant"
    )
    assert step_env.get("WINDOWS_VCPUS") == "${{ env.GITHUB_WINDOWS_VCPUS }}", (
        "the parallelism step must read the named Windows vCPU constant"
    )


def test_linux_lane_measures_memory_and_disk() -> None:
    """A shape decision needs the resource that actually constrains the job."""
    steps = _steps(_job("ci.yml", "build-test"))
    sampler = steps[_step_index(steps, "Start resource sampler")]
    assert sampler.get("if") == "${{ runner.os == 'Linux' }}", (
        "the sampler reads /proc and df, so it is Linux only"
    )
    sampler_script = str(sampler.get("run"))
    for fragment in ("free -m", "df -m --output=used,avail", "sleep 15"):
        assert fragment in sampler_script, (
            f"the resource sampler must contain {fragment!r}; memory alone "
            "cannot show the disk exhaustion that killed the publish step"
        )
    assert _step_index(steps, "Start resource sampler") > _step_index(
        steps, "Checkout"
    ), "the sampler starts after checkout so it covers the build and tests"

    report = steps[_step_index(steps, "Report peak resource use")]
    assert report.get("if") == "${{ always() && runner.os == 'Linux' }}", (
        "the peaks must be reported even when the job dies, which is the case "
        "they exist to diagnose"
    )
    report_script = str(report.get("run"))
    for fragment in (
        "peak used memory",
        "peak used disk",
        "least free disk",
        "GITHUB_STEP_SUMMARY",
    ):
        assert fragment in report_script, (
            f"the resource report must contain {fragment!r}"
        )
    printed = report_script[: report_script.index("GITHUB_STEP_SUMMARY")]
    assert "printf 'peak used disk" in printed, (
        "the peaks must reach the job log, not only the step summary, so a "
        "failed run can be diagnosed from the log alone"
    )


def test_publish_check_runs_without_the_spent_build_trees() -> None:
    """Several full workspace trees do not fit the shape's 72 GB disk."""
    steps = _steps(_job("ci.yml", "build-test"))
    discard_index = _step_index(steps, "Discard spent build trees")
    publish_index = _step_index(steps, "Publish dry run")
    assert discard_index < publish_index, (
        "the spent trees must go before the publish build creates its own"
    )
    coverage_index = _step_index(steps, "Test and Measure Coverage (Linux)")
    assert coverage_index < discard_index, (
        "the report must exist before its scratch tree is discarded"
    )
    discard = steps[discard_index]
    assert discard.get("if") == "${{ runner.os == 'Linux' }}", (
        "the discard uses df and rm, so it is Linux only"
    )
    script = str(discard.get("run"))
    for tree in (
        "target/llvm-cov-target",
        "target/debug",
        "tests/fixtures/published-gpui-0-2-2/target",
        "tests/fixtures/published-gpui-e2e/target",
    ):
        assert tree in script, (
            f"{tree!r} has no consumer past the coverage report, so the "
            "publish build must not have to share the disk with it"
        )
    assert script.count("df -h") >= 2, (
        "record the disk before and after the discard so the saving is measured"
    )
    assert publish_index == discard_index + 1, (
        "nothing may run between them, so the discard's closing reading is the "
        "disk the publish build starts with; the publish step itself cannot "
        "print one because it also runs under PowerShell on Windows"
    )
