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


def _step_run(workflow_name: str, job_name: str, step_name: str) -> str:
    """Return the shell program for one named workflow step."""
    steps = _job(workflow_name, job_name).get("steps")
    assert isinstance(steps, list), f"{workflow_name}:{job_name} must declare steps"
    step = next(
        (
            candidate
            for candidate in steps
            if isinstance(candidate, dict) and candidate.get("name") == step_name
        ),
        None,
    )
    assert isinstance(step, dict), (
        f"{workflow_name}:{job_name} must declare {step_name}"
    )
    run = step.get("run")
    assert isinstance(run, str), f"{workflow_name}:{step_name} must run a shell program"
    return run


def test_comment_job_uses_the_shared_uncached_github_hosted_profile() -> None:
    """Keep the controlled utility-job assignment from drifting."""
    job = _job("delayed-pr-comment.yml", "delay_and_comment")
    assert job.get("runs-on") == "ubuntu-latest", (
        "delayed-pr-comment.yml:delay_and_comment must stay on GitHub-hosted "
        "compute so it cannot consume paid Linux capacity"
    )
    assert job.get("timeout-minutes") == 65, (
        "delayed-pr-comment.yml:delay_and_comment must bound runner occupancy"
    )


def test_delayed_comment_validates_delay_boundaries_without_sleeping() -> None:
    """Keep 1 and 60 valid while rejecting malformed or out-of-range delays."""
    run = _step_run(
        "delayed-pr-comment.yml", "delay_and_comment", "Convert minutes to seconds"
    )
    required_fragments = (
        "''|*[!0-9]*)",
        "10#$DELAY_MINUTES < 1 || 10#$DELAY_MINUTES > 60",
        "10#$DELAY_MINUTES * 60",
    )
    missing_fragments = [
        fragment for fragment in required_fragments if fragment not in run
    ]
    assert not missing_fragments, (
        "delayed-pr-comment.yml:delay_and_comment must validate the delay "
        f"boundaries and calculate seconds; missing {missing_fragments}"
    )


def test_build_matrix_resolves_its_runner_from_the_matrix() -> None:
    """Keep the build matrix's runner selection data-driven, not hard-coded."""
    build_test = _job("ci.yml", "build-test")
    assert build_test.get("runs-on") == "${{ matrix.os }}", (
        "ci.yml:build-test must resolve its runner from matrix.os"
    )
