"""Hold the CodeScene token's environment to the uploading job (CV-005).

The token lives in the `codescene` environment, whose deployment policy admits
`main` alone. So every job that invokes the uploader declares that
environment, no other job does, and no workflow a pull request can start
declares it in any job: a declaration there would let branch code ask for the
token.

The module is self-contained: it reads the workflows with a loader that
refuses duplicate keys, finds what a pull request can start from each
workflow's triggers, and follows local reusable-workflow calls from there.
"""

import typing as typ

import yaml

if typ.TYPE_CHECKING:
    import collections.abc as cabc
    from pathlib import Path

type Workflow = dict[object, object]
type Job = dict[object, object]

ENVIRONMENT: typ.Final[str] = "codescene"
UPLOAD_ACTION: typ.Final[str] = "upload-codescene-coverage"
LOCAL_WORKFLOW: typ.Final[str] = "./.github/workflows/"
PULL_REQUEST_EVENTS: typ.Final[frozenset[str]] = frozenset({
    "pull_request",
    "pull_request_target",
    "pull_request_review",
    "pull_request_review_comment",
    "issue_comment",
    "merge_group",
})
TRUNK_BRANCHES: typ.Final[frozenset[str]] = frozenset({"main"})
MISSING: typ.Final[str] = f"the uploading job must declare `environment: {ENVIRONMENT}`"
STRAY: typ.Final[str] = f"declares `{ENVIRONMENT}` but uploads nothing"
REACHABLE: typ.Final[str] = (
    f"is reachable from a pull request and declares `{ENVIRONMENT}`"
)


class DuplicateKeyError(ValueError):
    """Raised when a workflow declares one mapping key twice."""


class _StrictLoader(yaml.SafeLoader):
    """A safe loader that refuses a key declared twice in one mapping."""


def _construct_mapping(loader: _StrictLoader, node: yaml.MappingNode) -> Workflow:
    """Build a mapping, raising on a repeated key.

    Returns
    -------
    Workflow
        The constructed mapping.

    Raises
    ------
    DuplicateKeyError
        If one key appears twice.

    """
    seen: set[object] = set()
    for key_node, _ in node.value:
        key = loader.construct_object(key_node, deep=True)
        if key in seen:
            message = f"duplicate key {key!r} at {key_node.start_mark}"
            raise DuplicateKeyError(message)
        seen.add(key)
    return loader.construct_mapping(node, deep=True)


_StrictLoader.add_constructor(
    yaml.resolver.BaseResolver.DEFAULT_MAPPING_TAG, _construct_mapping
)


def _load(text: str) -> object:
    """Parse one YAML document with the strict safe loader.

    Returns
    -------
    object
        The parsed document.

    """
    loader = _StrictLoader(text)
    try:
        return loader.get_single_data()
    finally:
        loader.dispose()


def read_workflows(directory: Path) -> dict[str, Workflow]:
    """Read every workflow in a directory, by file name.

    Returns
    -------
    dict[str, Workflow]
        Each parsed workflow mapping.

    Raises
    ------
    ValueError
        If the directory holds no workflow, which is a reader fault.

    """
    found = {
        path.name: _load(path.read_text(encoding="utf-8"))
        for path in sorted(directory.iterdir())
        if path.suffix in {".yml", ".yaml"}
    }
    workflows = {name: doc for name, doc in found.items() if isinstance(doc, dict)}
    if not workflows:
        message = f"no workflows under {directory}"
        raise ValueError(message)
    return workflows


def jobs(workflow: Workflow) -> dict[str, Job]:
    """Return a workflow's jobs that are mappings, by name.

    Returns
    -------
    dict[str, Job]
        Job id to job, in declaration order.

    """
    declared = workflow.get("jobs")
    if not isinstance(declared, dict):
        return {}
    return {str(key): job for key, job in declared.items() if isinstance(job, dict)}


def environment_name(job: Job) -> str | None:
    """Return the environment a job declares, from either accepted form.

    Returns
    -------
    str | None
        The environment's name, or None when the job declares none.

    Examples
    --------
    >>> environment_name({"environment": "codescene"})
    'codescene'
    >>> environment_name({"environment": {"name": "codescene", "url": "x"}})
    'codescene'
    >>> environment_name({}) is None
    True

    """
    match job.get("environment"):
        case str() as name:
            return name
        case {"name": str() as name}:
            return name
        case _:
            return None


def uploads(job: Job) -> bool:
    """Return whether a job has a step invoking the uploader.

    Returns
    -------
    bool
        True when some step's `uses` names the upload action.

    """
    declared = job.get("steps")
    listed = declared if isinstance(declared, list) else []
    return any(
        isinstance(step, dict) and UPLOAD_ACTION in str(step.get("uses", ""))
        for step in listed
    )


def _triggers(workflow: Workflow) -> dict[str, object]:
    """Return a workflow's triggers from any of their three forms.

    Returns
    -------
    dict[str, object]
        Event name to its filter, or None where none is given.

    """
    declared = workflow.get("on", workflow.get(True))
    match declared:
        case str() as event:
            return {event: None}
        case list() as events:
            return {str(event): None for event in events}
        case dict() as mapping:
            return {str(event): value for event, value in mapping.items()}
        case _:
            return {}


def _push_is_trunk_only(push: object) -> bool:
    """Return whether a push trigger is confined to main or to tags.

    Returns
    -------
    bool
        True when branch filters name main alone, or only tags are pushed.

    """
    if not isinstance(push, dict):
        return False
    branches = push.get("branches")
    if branches is None:
        return "tags" in push and "branches-ignore" not in push
    return isinstance(branches, list) and set(map(str, branches)) <= TRUNK_BRANCHES


def starts_on_pull_request(workflow: Workflow) -> bool:
    """Return whether a pull request can start a workflow by its own triggers.

    Returns
    -------
    bool
        True for a pull-request event, or a push not confined to main or tags.

    """
    events = _triggers(workflow)
    if PULL_REQUEST_EVENTS & events.keys():
        return True
    return "push" in events and not _push_is_trunk_only(events["push"])


def _local_callees(workflow: Workflow) -> set[str]:
    """Return the local workflows a workflow's jobs call, by file name.

    Returns
    -------
    set[str]
        File names under `.github/workflows/`.

    """
    return {
        str(job["uses"]).removeprefix(LOCAL_WORKFLOW)
        for job in jobs(workflow).values()
        if str(job.get("uses", "")).startswith(LOCAL_WORKFLOW)
    }


def pull_request_closure(workflows: dict[str, Workflow]) -> set[str]:
    """Return every workflow a pull request can start, directly or not.

    Returns
    -------
    set[str]
        The seeds and every local workflow they call, transitively.

    Raises
    ------
    ValueError
        If no workflow serves a pull request, which is a reader fault.

    """
    reached = {name for name, flow in workflows.items() if starts_on_pull_request(flow)}
    if not reached:
        message = "no workflow serves a pull request; the reader is broken"
        raise ValueError(message)
    pending = list(reached)
    while pending:
        fresh = (_local_callees(workflows[pending.pop()]) & workflows.keys()) - reached
        reached |= fresh
        pending.extend(fresh)
    return reached


def _placed(
    workflows: dict[str, Workflow], names: cabc.Iterable[str]
) -> list[tuple[str, Job]]:
    """Return every job in the named workflows with its location.

    Returns
    -------
    list[tuple[str, Job]]
        `"workflow:job"` and the job, for each job.

    """
    return [
        (f"{name}:{job_id}", job)
        for name in sorted(names)
        for job_id, job in jobs(workflows[name]).items()
    ]


def environment_violations(workflows: dict[str, Workflow]) -> list[str]:
    """Report every departure from the `codescene` environment placement.

    Returns
    -------
    list[str]
        One message per violation; empty when the placement holds.

    """
    placed = _placed(workflows, workflows)
    uploading = [(where, job) for where, job in placed if uploads(job)]
    if not uploading:
        return ["no workflow job invokes the CodeScene uploader"]
    problems = [
        f"{where}: {MISSING}"
        for where, job in uploading
        if environment_name(job) != ENVIRONMENT
    ]
    problems.extend(
        f"{where} {STRAY}"
        for where, job in placed
        if not uploads(job) and environment_name(job) == ENVIRONMENT
    )
    problems.extend(
        f"{where} {REACHABLE}"
        for where, job in _placed(workflows, pull_request_closure(workflows))
        if environment_name(job) == ENVIRONMENT
    )
    return problems
