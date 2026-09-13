"""Shared workflow document loaders for the contract modules.

:mod:`runner_placement_test`, :mod:`runner_cache_test` and the other contracts
read the same workflow documents. Keeping the loaders and the deployed runner
constants in one place stops those modules from drifting apart; the cache-step
anatomy they build on lives in :mod:`cache_step_support`.

The helpers raise subclasses of :class:`WorkflowShapeError` rather than
asserting, so the module carries no blanket lint suppression and a malformed
workflow fails the same way whether or not assertions are enabled.
"""

from pathlib import Path

import yaml

ROOT = Path(__file__).resolve().parents[2]
GITHUB_HOSTED_LINUX = "ubuntu-latest"
UBICLOUD_LINUX_LABEL = "ubicloud-standard-2"
GITHUB_HOSTED_WINDOWS = "windows-latest"
# The named vCPU constants for the two deployed shapes. Build and test
# parallelism is derived from these and must never exceed them.
UBICLOUD_LINUX_VCPUS = "2"
GITHUB_WINDOWS_VCPUS = "4"
SCCACHE_DIRECTORY = "${{ github.workspace }}/.sccache"


class WorkflowShapeError(AssertionError):
    """Base for the shape violations these contracts can detect.

    Derives from :class:`AssertionError` so a contract failure reads as a
    failed expectation in test output rather than an unexpected crash.
    """


class NotAMappingError(WorkflowShapeError):
    """A document or fragment that should have parsed to a mapping did not."""

    def __init__(self, subject: str) -> None:
        super().__init__(f"{subject} must parse to a mapping")


class UnparsableWorkflowError(WorkflowShapeError):
    """A workflow document is not parsable YAML.

    Raised by :func:`parse_workflow` when ``yaml.safe_load`` raises, so
    a malformed workflow fails as a shape violation with the same
    treatment as any other contract failure rather than as a raw parser
    fault.

    Parameters
    ----------
    subject : str
        What was being parsed, named in the error message.

    See Also
    --------
    parse_workflow : The parsing boundary that raises this.
    """

    def __init__(self, subject: str) -> None:
        super().__init__(f"{subject} is not parsable YAML")


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
    """A file a contract reads is not in the repository.

    Raised by :func:`repository_file` rather than letting an
    ``OSError`` escape, so a contract that names a file which has moved
    fails as a shape violation with the path in the message, the same
    way a malformed workflow does.

    Parameters
    ----------
    subject : str
        The path, relative to the repository root, that was not found.
        Rendered natively, so a Windows reader sees a Windows path.

    See Also
    --------
    repository_file : The reading boundary that raises this.
    """

    def __init__(self, subject: str) -> None:
        super().__init__(f"{subject} must exist in the repository")


class RepositoryReadError(WorkflowShapeError):
    """A repository file exists but could not be read as UTF-8 text.

    The single reading boundary (:func:`repository_file`) turns every
    other failure of the filesystem or the decoder into this one shape
    violation, so a contract that names an unreadable file fails with
    the path and the failure category instead of a raw ``OSError``.

    Parameters
    ----------
    subject : str
        The path, relative to the repository root, that was unreadable.
        Rendered natively, so a Windows reader sees a Windows path.
    category : str
        Human-readable failure category, e.g. ``permission denied``.

    See Also
    --------
    repository_file : The reading boundary that raises this.
    """

    def __init__(self, subject: str, category: str) -> None:
        self.category = category
        super().__init__(f"{subject} could not be read: {category}")


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

    Any failure is a :class:`workflow_support.WorkflowShapeError` raised
    by :func:`repository_file` (absent or unreadable file) or by
    :func:`parse_workflow` (non-mapping document).
    """
    return parse_workflow(repository_file(".github", "workflows", workflow_name))


def parse_workflow(workflow_text: str) -> dict[str, object]:
    """Parse one workflow document from its YAML text.

    A pure function: no repository access. The reader half of the split
    is :func:`workflow`, which reads the file through
    :func:`repository_file` and hands the text here.

    Parameters
    ----------
    workflow_text : str
        The raw YAML text of one workflow.

    Returns
    -------
    dict[str, object]
        The parsed workflow mapping.

    Raises
    ------
    UnparsableWorkflowError
        If the document is not parsable YAML.
    NotAMappingError
        If the document does not parse to a mapping.
    """
    try:
        document = yaml.safe_load(workflow_text)
    except yaml.YAMLError:
        raise UnparsableWorkflowError("workflow") from None
    if not isinstance(document, dict):
        raise NotAMappingError("workflow")
    return document


def job_from_document(document: dict[str, object], job_name: str) -> dict[str, object]:
    """Extract one named job from a parsed workflow mapping.

    A pure function: no repository access. The repository-backed form
    is :func:`job`.

    Parameters
    ----------
    document : dict[str, object]
        The parsed workflow mapping, as returned by :func:`parse_workflow`.
    job_name : str
        Key of the job within the workflow's ``jobs`` mapping.

    Returns
    -------
    dict[str, object]
        The parsed job mapping.

    Raises
    ------
    MissingKeyError
        If the workflow does not declare the named job.
    """
    declared = document.get("jobs")
    if not isinstance(declared, dict):
        raise MissingKeyError("workflow", "jobs")
    selected = declared.get(job_name)
    if not isinstance(selected, dict):
        raise MissingKeyError("workflow", job_name)
    return selected


def repository_file(*parts: str) -> str:
    """Return the text of one file in the repository.

    The single reading boundary for contracts that assert on files other
    than workflows. Contracts that opened paths themselves each grew
    their own root, encoding and error handling, and a contract with its
    own file access has no boundary to test at.

    Every failure is reported as a :class:`WorkflowShapeError`: an
    absent file raises :class:`MissingRepositoryFileError`, and every
    other filesystem or decoding failure raises
    :class:`RepositoryReadError` carrying the failure category, so no
    raw ``OSError`` or ``UnicodeDecodeError`` escapes this helper API.

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
    RepositoryReadError
        If the file exists but cannot be read as UTF-8 text.
    """
    path = ROOT.joinpath(*parts)
    subject = str(Path(*parts))
    try:
        return path.read_text(encoding="utf-8")
    except FileNotFoundError, IsADirectoryError, NotADirectoryError:
        raise MissingRepositoryFileError(subject) from None
    except PermissionError:
        raise RepositoryReadError(subject, "permission denied") from None
    except UnicodeDecodeError:
        raise RepositoryReadError(subject, "not valid UTF-8 text") from None
    except OSError as read_error:
        raise RepositoryReadError(subject, f"unreadable ({read_error})") from None


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

    Fails with :class:`workflow_support.MissingKeyError` if the workflow
    does not declare the named job.
    """
    return job_from_document(workflow(workflow_name), job_name)


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
