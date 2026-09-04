"""Flattened queries over the workflow estate.

The contracts assert one fact each. Walking every workflow, job, and step to
find the facts is a separate concern, and doing it inline turns each contract
into three nested loops. These generators flatten that traversal so a contract
reads as a list comprehension and one assertion.
"""

import dataclasses
import typing as typ

from workflow_support import (
    ROOT,
    cache_owner,
    cache_paths,
    is_cache_step,
    jobs,
    path_components,
    runs_workspace_tests,
    steps,
)

if typ.TYPE_CHECKING:
    import collections.abc as cabc

SHARED_CACHE_OWNING_ACTIONS = ("setup-rust", "generate-coverage")
# GitHub accepts either extension, so a contract that scans one is a gap.
WORKFLOW_SUFFIXES = (".yml", ".yaml")


@dataclasses.dataclass(frozen=True, slots=True)
class StepRef:
    """One step, with enough context to name it in a failure message."""

    workflow: str
    job: str
    step: dict[str, object]

    @property
    def name(self) -> str:
        """The step's declared name.

        Returns
        -------
        str
            The step name, or the empty string when it has none.
        """
        return str(self.step.get("name", ""))

    @property
    def uses(self) -> str:
        """The action reference the step invokes.

        Returns
        -------
        str
            The ``uses`` value, or the empty string for a ``run`` step.
        """
        return str(self.step.get("uses", ""))

    def __str__(self) -> str:
        """Return a location suitable for a failure message.

        Returns
        -------
        str
            ``workflow:job:step`` for this step.
        """
        return f"{self.workflow}:{self.job}:{self.name!r}"


def workflow_names() -> list[str]:
    """Return every workflow file name in the repository.

    Returns
    -------
    list[str]
        Sorted workflow file names.
    """
    directory = ROOT / ".github" / "workflows"
    return sorted(
        path.name
        for suffix in WORKFLOW_SUFFIXES
        for path in directory.glob(f"*{suffix}")
    )


def iter_steps(workflow_name: str | None = None) -> cabc.Iterator[StepRef]:
    """Yield every step of every job, optionally within one workflow.

    Jobs that call a reusable workflow declare no steps and are skipped.

    Parameters
    ----------
    workflow_name : str | None
        Restrict the walk to this workflow, or walk them all when None.

    Yields
    ------
    StepRef
        Each step, with its workflow and job name attached.
    """
    names = [workflow_name] if workflow_name else workflow_names()
    for name in names:
        for job_name, job_document in jobs(name).items():
            if "steps" not in job_document:
                continue
            for step in steps(job_document):
                yield StepRef(name, job_name, step)


def cache_steps(workflow_name: str, job_name: str) -> list[StepRef]:
    """Return every cache step of one job.

    Parameters
    ----------
    workflow_name : str
        File name of the workflow.
    job_name : str
        Key of the job within that workflow.

    Returns
    -------
    list[StepRef]
        The job's cache steps, in declaration order.
    """
    return [
        ref
        for ref in iter_steps(workflow_name)
        if ref.job == job_name and is_cache_step(ref.step)
    ]


def archived_target_paths(
    workflow_name: str, job_name: str
) -> list[tuple[StepRef, str]]:
    """Return every cached path holding a ``target`` component.

    Parameters
    ----------
    workflow_name : str
        File name of the workflow.
    job_name : str
        Key of the job within that workflow.

    Returns
    -------
    list[tuple[StepRef, str]]
        Each offending step paired with the path that offends.
    """
    return [
        (ref, path)
        for ref in cache_steps(workflow_name, job_name)
        for path in cache_paths(ref.step)
        if "target" in path_components(path)
    ]


def owned_paths(workflow_name: str, job_name: str) -> dict[str, set[str]]:
    """Return the cached paths each logical owner claims.

    Parameters
    ----------
    workflow_name : str
        File name of the workflow.
    job_name : str
        Key of the job within that workflow.

    Returns
    -------
    dict[str, set[str]]
        Owner name to the set of paths it claims.
    """
    owners: dict[str, set[str]] = {}
    for ref in cache_steps(workflow_name, job_name):
        owners.setdefault(cache_owner(ref.step), set()).update(cache_paths(ref.step))
    return owners


def is_shared_cache_owning_action(ref: StepRef) -> bool:
    """Report whether a step calls a shared action that can own a cache.

    Parameters
    ----------
    ref : StepRef
        One step.

    Returns
    -------
    bool
        True when the step invokes ``setup-rust`` or ``generate-coverage``.
    """
    return any(
        ref.uses.startswith(f"leynos/shared-actions/.github/actions/{name}@")
        for name in SHARED_CACHE_OWNING_ACTIONS
    )


def shared_cache_owning_steps() -> list[StepRef]:
    """Return every call to a shared action that could own a cache.

    Returns
    -------
    list[StepRef]
        Steps across all workflows invoking such an action.
    """
    return [ref for ref in iter_steps() if is_shared_cache_owning_action(ref)]


def external_provider_violations() -> list[str]:
    """Return the shared-action calls that do not cede cache ownership.

    Returns
    -------
    list[str]
        A location string per offending step.
    """
    violations: list[str] = []
    for ref in shared_cache_owning_steps():
        inputs = ref.step.get("with")
        provider = inputs.get("cache-provider") if isinstance(inputs, dict) else None
        if provider != "external":
            violations.append(str(ref))
    return violations


def coverage_calling_jobs() -> list[str]:
    """Return each job that invokes the shared coverage action.

    Returns
    -------
    list[str]
        ``workflow:job`` per calling job, without repeats.
    """
    callers: list[str] = []
    for ref in iter_steps():
        caller = f"{ref.workflow}:{ref.job}"
        calls_coverage = ref.uses.startswith(
            "leynos/shared-actions/.github/actions/generate-coverage@"
        )
        if calls_coverage and caller not in callers:
            callers.append(caller)
    return callers


def direct_workspace_test_steps() -> list[str]:
    """Return every step invoking a workspace test driver directly.

    Returns
    -------
    list[str]
        A location string per offending step.
    """
    return [str(ref) for ref in iter_steps() if runs_workspace_tests(ref.step)]
