"""Shared loading helpers for the workflow runner and cache contracts.

Both :mod:`runner_placement_test` and :mod:`runner_cache_test` read the same
workflow documents. Keeping the loaders and the deployed runner constants in
one place stops the two contract modules from drifting apart.

The helpers raise subclasses of :class:`WorkflowShapeError` rather than
asserting, so the module carries no blanket lint suppression and a malformed
workflow fails the same way whether or not assertions are enabled.
"""

import re
from pathlib import Path, PurePosixPath, PureWindowsPath

import yaml

ROOT = Path(__file__).resolve().parents[2]
GITHUB_HOSTED_LINUX = "ubuntu-latest"
UBICLOUD_LINUX_LABEL = "ubicloud-standard-2"
GITHUB_HOSTED_WINDOWS = "windows-latest"
# Ubicloud's transparent cache intercepts actions/cache v6.1.0 on Linux and
# GitHub serves it on Windows, verified against the Ubicloud cache listing on
# 2026-09-03. One action and one pin therefore serve every lane.
CACHE_ACTION_PREFIX = "actions/cache/"
CACHE_ACTION_REF = "@55cc8345863c7cc4c66a329aec7e433d2d1c52a9"
SHARED_SETUP_RUST_CACHE_PROVIDER_HEAD = "5daae0a332441d170d88ca648c9e71f0bbe96cb3"
# The named vCPU constants for the two deployed shapes. Build and test
# parallelism is derived from these and must never exceed them.
UBICLOUD_LINUX_VCPUS = "2"
GITHUB_WINDOWS_VCPUS = "4"
SCCACHE_DIRECTORY = "${{ github.workspace }}/.sccache"
# Commands that execute the Rust workspace suite. `make test-workflow-contracts`
# is deliberately excluded: it runs this Python suite, not the workspace.
WORKSPACE_TEST_COMMANDS = ("cargo test", "cargo nextest", "make test")


class WorkflowShapeError(AssertionError):
    """Base for the shape violations these contracts can detect.

    Derives from :class:`AssertionError` so a contract failure reads as a
    failed expectation in test output rather than an unexpected crash.
    """


class NotAMappingError(WorkflowShapeError):
    """A document or fragment that should have parsed to a mapping did not."""

    def __init__(self, subject: str) -> None:
        super().__init__(f"{subject} must parse to a mapping")


class MissingKeyError(WorkflowShapeError):
    """A document did not declare a structure the contracts require."""

    def __init__(self, subject: str, expected: str) -> None:
        super().__init__(f"{subject} must declare {expected}")


class StepNotAMappingError(WorkflowShapeError):
    """A workflow step was not a mapping."""

    def __init__(self) -> None:
        super().__init__("every workflow step must be a mapping")


class CacheStepInputsError(WorkflowShapeError):
    """A cache step declared no inputs."""

    def __init__(self) -> None:
        super().__init__("a cache step must declare inputs")


class CacheStepPathsError(WorkflowShapeError):
    """A cache step declared no paths."""

    def __init__(self) -> None:
        super().__init__("a cache step must declare its paths")


class MissingRepositoryFileError(WorkflowShapeError):
    """A file the contracts read is not in the repository."""

    def __init__(self, subject: str) -> None:
        super().__init__(f"{subject} must exist in the repository")


class AmbiguousStepError(WorkflowShapeError):
    """A step name did not match exactly one step in a job."""

    def __init__(self, name: str, found: int) -> None:
        super().__init__(f"expected exactly one {name!r} step, found {found}")


def workflow(workflow_name: str) -> dict[str, object]:
    """Load and validate one repository workflow document.

    Parameters
    ----------
    workflow_name : str
        File name of the workflow under ``.github/workflows``.

    Returns
    -------
    dict[str, object]
        The parsed workflow mapping.

    Raises
    ------
    NotAMappingError
        If the document does not parse to a mapping.
    """
    workflow_path = ROOT / ".github" / "workflows" / workflow_name
    document = yaml.safe_load(workflow_path.read_text(encoding="utf-8"))
    if not isinstance(document, dict):
        raise NotAMappingError(workflow_name)
    return document


def repository_file(*parts: str) -> str:
    """Return the text of one file in the repository.

    The single reading boundary for contracts that assert on files other
    than workflows. Contracts that opened paths themselves each grew
    their own root, encoding and error handling, and a contract with its
    own file access has no boundary to test at.

    Parameters
    ----------
    *parts : str
        Path components below the repository root.

    Returns
    -------
    str
        The file's contents.

    Raises
    ------
    MissingRepositoryFileError
        If no such file exists.
    """
    path = ROOT.joinpath(*parts)
    if not path.is_file():
        raise MissingRepositoryFileError("/".join(parts))
    return path.read_text(encoding="utf-8")


def jobs(workflow_name: str) -> dict[str, dict[str, object]]:
    """Return every job declared by one workflow.

    Parameters
    ----------
    workflow_name : str
        File name of the workflow under ``.github/workflows``.

    Returns
    -------
    dict[str, dict[str, object]]
        Job name to job mapping.

    Raises
    ------
    MissingKeyError
        If the workflow declares no jobs mapping.
    """
    document = workflow(workflow_name)
    declared = document.get("jobs")
    if not isinstance(declared, dict):
        raise MissingKeyError(workflow_name, "jobs")
    return declared


def job(workflow_name: str, job_name: str) -> dict[str, object]:
    """Load one named job from a repository workflow.

    Parameters
    ----------
    workflow_name : str
        File name of the workflow under ``.github/workflows``.
    job_name : str
        Key of the job within that workflow's ``jobs`` mapping.

    Returns
    -------
    dict[str, object]
        The parsed job mapping.

    Raises
    ------
    MissingKeyError
        If the workflow does not declare the named job.
    """
    selected = jobs(workflow_name).get(job_name)
    if not isinstance(selected, dict):
        raise MissingKeyError(workflow_name, job_name)
    return selected


def steps(job_document: dict[str, object]) -> list[dict[str, object]]:
    """Return a job's steps after validating their mapping shape.

    Parameters
    ----------
    job_document : dict[str, object]
        A job mapping, as returned by :func:`job`.

    Returns
    -------
    list[dict[str, object]]
        Every step of the job, in declaration order.

    Raises
    ------
    MissingKeyError
        If the job declares no steps list.
    StepNotAMappingError
        If a step is not a mapping.
    """
    raw_steps = job_document.get("steps")
    if not isinstance(raw_steps, list):
        raise MissingKeyError("job", "a steps list")
    parsed: list[dict[str, object]] = []
    for raw_step in raw_steps:
        if not isinstance(raw_step, dict):
            raise StepNotAMappingError
        parsed.append(raw_step)
    return parsed


def step_index(job_steps: list[dict[str, object]], name: str) -> int:
    """Return the index of the uniquely named step.

    Parameters
    ----------
    job_steps : list[dict[str, object]]
        Steps of one job, as returned by :func:`steps`.
    name : str
        Exact ``name`` of the step to locate.

    Returns
    -------
    int
        The position of the named step.

    Raises
    ------
    AmbiguousStepError
        If the name does not match exactly one step.
    """
    matches = [
        index for index, step in enumerate(job_steps) if step.get("name") == name
    ]
    if len(matches) != 1:
        raise AmbiguousStepError(name, len(matches))
    return matches[0]


def is_cache_step(step: dict[str, object]) -> bool:
    """Report whether a step invokes one of the approved cache actions.

    Parameters
    ----------
    step : dict[str, object]
        One workflow step.

    Returns
    -------
    bool
        True when the step uses an approved cache action.
    """
    return str(step.get("uses", "")).startswith(CACHE_ACTION_PREFIX)


def cache_paths(step: dict[str, object]) -> list[str]:
    """Return the normalized cache paths a cache step owns.

    Parameters
    ----------
    step : dict[str, object]
        A cache step, as identified by :func:`is_cache_step`.

    Returns
    -------
    list[str]
        One entry per declared path, stripped of surrounding whitespace.

    Raises
    ------
    CacheStepInputsError
        If the step declares no inputs.
    CacheStepPathsError
        If the step declares no path.
    """
    inputs = step.get("with")
    if not isinstance(inputs, dict):
        raise CacheStepInputsError
    raw_path = inputs.get("path")
    if not isinstance(raw_path, str):
        raise CacheStepPathsError
    return [line.strip() for line in raw_path.splitlines() if line.strip()]


def path_components(path: str) -> list[str]:
    """Return every component of a cache path, under either separator.

    A workflow path may use POSIX or Windows separators, and a component such
    as ``target`` can sit at any depth, so both forms are parsed and merged.

    Parameters
    ----------
    path : str
        One declared cache path, possibly containing an expression.

    Returns
    -------
    list[str]
        Every path component under both separator conventions.
    """
    # Expressions such as ${{ github.workspace }} contain no separator of
    # interest, so they survive as a single component either way.
    posix = PurePosixPath(path).parts
    windows = PureWindowsPath(path).parts
    return [part.strip("/\\") for part in (*posix, *windows) if part.strip("/\\")]


def cache_owner(step: dict[str, object]) -> str:
    """Return the logical owner name of a cache step.

    Restore and save steps for the same paths share an owner, and so do the
    Linux and Windows variants of one owner: their ``runner.os`` guards make
    them mutually exclusive.

    Parameters
    ----------
    step : dict[str, object]
        A cache step, as identified by :func:`is_cache_step`.

    Returns
    -------
    str
        The step name without its action verb or its runner-provider suffix.
    """
    name = str(step.get("name", ""))
    name = re.sub(r"^(Restore|Save) ", "", name)
    return re.sub(r"\s*\([^)]*\)$", "", name)


def runs_workspace_tests(step: dict[str, object]) -> bool:
    """Report whether a step runs the Rust workspace suite directly.

    Parameters
    ----------
    step : dict[str, object]
        One workflow step.

    Returns
    -------
    bool
        True when the step's script invokes a workspace test driver.
    """
    script = str(step.get("run", ""))
    for line in script.splitlines():
        command = line.strip()
        for driver in WORKSPACE_TEST_COMMANDS:
            if command == driver or command.startswith(f"{driver} "):
                return True
    return False
