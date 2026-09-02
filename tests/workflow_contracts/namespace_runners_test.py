"""Verify GitHub Actions runner assignments.

Load workflow YAML and check runner contracts for the delayed comment job
and the build matrix.

Run with:

    pytest tests/workflow_contracts/namespace_runners_test.py
"""

from pathlib import Path

import yaml

ROOT = Path(__file__).resolve().parents[2]


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


def test_comment_job_uses_the_shared_uncached_namespace_profile() -> None:
    """Keep the controlled utility-job assignment from drifting."""
    job = _job("delayed-pr-comment.yml", "delay_and_comment")
    assert job.get("runs-on") == ("namespace-profile-default"), (
        "delayed-pr-comment.yml:delay_and_comment must use namespace-profile-default"
    )
    assert job.get("timeout-minutes") == 65, (
        "delayed-pr-comment.yml:delay_and_comment must bound runner occupancy"
    )


def test_build_matrix_remains_platform_controlled() -> None:
    """Keep Windows and capacity-sensitive Linux legs on their current runners."""
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
    matrix_oses = {entry.get("os") for entry in include if isinstance(entry, dict)}
    assert {"ubuntu-latest", "windows-latest"} <= matrix_oses, (
        "ci.yml:build-test matrix must retain ubuntu-latest and windows-latest"
    )
