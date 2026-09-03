"""Shared loading helpers for the workflow runner and cache contracts.

Both :mod:`runner_placement_test` and :mod:`runner_cache_test` read the same
workflow documents. Keeping the loaders and the deployed runner constants in
one place stops the two contract modules from drifting apart.
"""

import re
from pathlib import Path

import yaml

ROOT = Path(__file__).resolve().parents[2]
GITHUB_HOSTED_LINUX = "ubuntu-latest"
UBICLOUD_LINUX_LABEL = "ubicloud-standard-2"
GITHUB_HOSTED_WINDOWS = "windows-latest"
UBICLOUD_CACHE_PREFIX = "ubicloud/cache/"
GITHUB_CACHE_PREFIX = "actions/cache/"
UBICLOUD_CACHE_REF = "@92361f338d82d2c58a98875f1b5c95cd14cd6b2a"
GITHUB_CACHE_REF = "@55cc8345863c7cc4c66a329aec7e433d2d1c52a9"
SHARED_SETUP_RUST_CACHE_PROVIDER_HEAD = "5daae0a332441d170d88ca648c9e71f0bbe96cb3"
# The named vCPU constants for the two deployed shapes. Build and test
# parallelism is derived from these and must never exceed them.
UBICLOUD_LINUX_VCPUS = "2"
GITHUB_WINDOWS_VCPUS = "4"
SCCACHE_DIRECTORY = "${{ github.workspace }}/.sccache"


def workflow(workflow_name: str) -> dict[str, object]:
    """Load and validate one repository workflow document.

    Returns
    -------
    dict[str, object]
        The parsed workflow mapping.
    """
    workflow_path = ROOT / ".github" / "workflows" / workflow_name
    document = yaml.safe_load(workflow_path.read_text(encoding="utf-8"))
    assert isinstance(document, dict), f"{workflow_name} must parse to a mapping"
    return document


def job(workflow_name: str, job_name: str) -> dict[str, object]:
    """Load one named job from a repository workflow.

    Returns
    -------
    dict[str, object]
        The parsed job mapping.
    """
    document = workflow(workflow_name)
    jobs = document.get("jobs")
    assert isinstance(jobs, dict), f"{workflow_name} must declare jobs"
    selected = jobs.get(job_name)
    assert isinstance(selected, dict), f"{workflow_name} must declare {job_name}"
    return selected


def steps(job_document: dict[str, object]) -> list[dict[str, object]]:
    """Return a job's steps after validating their mapping shape.

    Returns
    -------
    list[dict[str, object]]
        Every step of the job, in declaration order.
    """
    raw_steps = job_document.get("steps")
    assert isinstance(raw_steps, list), "job must declare a steps list"
    parsed: list[dict[str, object]] = []
    for raw_step in raw_steps:
        assert isinstance(raw_step, dict), "every workflow step must be a mapping"
        parsed.append(raw_step)
    return parsed


def step_index(job_steps: list[dict[str, object]], name: str) -> int:
    """Return the index of the uniquely named step.

    Returns
    -------
    int
        The position of the named step.
    """
    matches = [
        index for index, step in enumerate(job_steps) if step.get("name") == name
    ]
    assert len(matches) == 1, (
        f"expected exactly one {name!r} step, found {len(matches)}"
    )
    return matches[0]


def is_cache_step(step: dict[str, object]) -> bool:
    """Report whether a step invokes one of the approved cache actions.

    Returns
    -------
    bool
        True when the step uses an approved cache action.
    """
    uses = str(step.get("uses", ""))
    return uses.startswith((UBICLOUD_CACHE_PREFIX, GITHUB_CACHE_PREFIX))


def cache_paths(step: dict[str, object]) -> list[str]:
    """Return the normalized cache paths a cache step owns.

    Returns
    -------
    list[str]
        One entry per declared path, stripped of surrounding whitespace.
    """
    inputs = step.get("with")
    assert isinstance(inputs, dict), "a cache step must declare inputs"
    raw_path = inputs.get("path")
    assert isinstance(raw_path, str), "a cache step must declare its paths"
    return [line.strip() for line in raw_path.splitlines() if line.strip()]


def cache_owner(step: dict[str, object]) -> str:
    """Return the logical owner name of a cache step.

    Restore and save steps for the same paths share an owner, and so do the
    Ubicloud and GitHub-hosted variants of one owner: their ``runner.os``
    guards make them mutually exclusive, which
    ``test_cache_actions_are_pinned_per_runner_provider`` enforces.

    Returns
    -------
    str
        The step name without its action verb or its runner-provider suffix.
    """
    name = str(step.get("name", ""))
    name = re.sub(r"^(Restore|Save) ", "", name)
    return re.sub(r"\s*\([^)]*\)$", "", name)
