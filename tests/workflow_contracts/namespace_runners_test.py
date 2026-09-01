"""Contract-test rstest-bdd's initial Namespace runner assignment."""

from __future__ import annotations

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
    assert _job("delayed-pr-comment.yml", "delay_and_comment").get("runs-on") == (
        "namespace-profile-default"
    )


def test_build_matrix_remains_platform_controlled() -> None:
    """Keep Windows and capacity-sensitive Linux legs on their current runners."""
    build_test = _job("ci.yml", "build-test")
    assert build_test.get("runs-on") == "${{ matrix.os }}"
