"""Keep every check name independent of the runner it happened to land on.

GitHub derives a matrix job's check name from its matrix values, with ``os``
first, so a derived name carries whichever runner label the event selected.
The Linux lane's label is now a conditional expression, so a fork pull request
would report a context name that no branch-protection rule can require, and
the required context would simply never arrive. The derived name is also long
enough that GitHub truncates it, which is how one of this repository's three
required contexts came to end in a literal ``...``.

An explicit ``name`` fixes that only while it stays clear of the runner. These
contracts hold all four halves of the rule: a matrix job declares a name, the
name shares no expression reference with ``runs-on``, it embeds no runner
label, and the names its matrix rows render stay distinct.

Run with:

    pytest tests/workflow_contracts/job_name_shape_test.py
"""

import re
import typing as typ

import pytest
from runner_label_support import RESOLVED_LINUX_LABELS
from workflow_queries import workflow_names as _workflow_names
from workflow_support import GITHUB_HOSTED_WINDOWS
from workflow_support import jobs as _jobs

#: Every label a job in this repository can run on. A name containing one of
#: these is keyed on the runner even when it interpolates nothing.
RUNNER_LABELS = (*RESOLVED_LINUX_LABELS, GITHUB_HOSTED_WINDOWS)
_EXPRESSION = re.compile(r"\$\{\{(?P<body>.*?)\}\}", re.DOTALL)
_REFERENCE = re.compile(r"[A-Za-z_][A-Za-z0-9_-]*(?:\.[A-Za-z_][A-Za-z0-9_-]*)+")


def _references(text: object) -> set[str]:
    """Return the context references an expression-bearing value reads.

    Parameters
    ----------
    text : object
        A workflow value, which YAML permits to be anything.

    Returns
    -------
    set[str]
        Dotted references such as ``matrix.os``, empty when the value
        is not a string or interpolates nothing.

    Examples
    --------
    >>> sorted(_references("${{ matrix.os }}"))
    ['matrix.os']
    >>> _references("ubuntu-latest")
    set()
    """
    if not isinstance(text, str):
        return set()
    found: set[str] = set()
    for match in _EXPRESSION.finditer(text):
        found.update(_REFERENCE.findall(match["body"]))
    return found


def _render(name: str, row: dict[str, object]) -> str:
    """Render a job name against one matrix include row.

    Parameters
    ----------
    name : str
        The job's declared ``name``.
    row : dict[str, object]
        One matrix include row.

    Returns
    -------
    str
        The name with every ``matrix.<key>`` the row supplies substituted.

    Examples
    --------
    >>> _render("t (${{ matrix.platform }})", {"platform": "linux"})
    't (linux)'
    """

    def substitute(match: re.Match[str]) -> str:
        reference = match["body"].strip()
        key = reference.removeprefix("matrix.")
        return str(row[key]) if key in row else match.group(0)

    return _EXPRESSION.sub(substitute, name)


def _matrix_rows(job_document: dict[str, object]) -> list[dict[str, object]]:
    """Return a job's matrix include rows that are mappings.

    Parameters
    ----------
    job_document : dict[str, object]
        A job mapping.

    Returns
    -------
    list[dict[str, object]]
        The include rows, empty when the job declares no matrix.
    """
    strategy = job_document.get("strategy")
    matrix = strategy.get("matrix") if isinstance(strategy, dict) else None
    include = matrix.get("include") if isinstance(matrix, dict) else None
    rows = include if isinstance(include, list) else []
    return [row for row in rows if isinstance(row, dict)]


class JobRef(typ.NamedTuple):
    """One job, with enough context to name it in a failure.

    Attributes
    ----------
    workflow : str
        File name of the declaring workflow.
    name : str
        The job key.
    document : dict[str, object]
        The job mapping.
    """

    workflow: str
    name: str
    document: dict[str, object]

    @property
    def where(self) -> str:
        """A human-readable location for a failure message.

        Returns
        -------
        str
            The workflow and job key.
        """
        return f"{self.workflow}:{self.name}"


def _matrix_jobs() -> list[JobRef]:
    """Return every job in the estate that declares a matrix.

    Returns
    -------
    list[JobRef]
        Each matrix job, in workflow and declaration order.
    """
    return [
        JobRef(workflow_name, job_name, job_document)
        for workflow_name in _workflow_names()
        for job_name, job_document in _jobs(workflow_name).items()
        if _matrix_rows(job_document)
    ]


def test_the_estate_declares_at_least_one_matrix_job() -> None:
    """Refuse a vacuous pass.

    Every contract below is a list comprehension over the matrix jobs, so a
    traversal that found none would report success while asserting nothing.
    """
    assert _matrix_jobs(), (
        "the contracts in this module are vacuous unless the traversal finds "
        "a matrix job; ci.yml:build-test is one"
    )


def test_every_matrix_job_declares_an_explicit_name() -> None:
    """Refuse a name GitHub derives from the matrix.

    A derived name lists the matrix values with ``os`` first, so it carries
    the runner label and changes with the event. Deleting the declared name
    restores that silently, which is what this refuses.
    """
    unnamed = [
        job.where
        for job in _matrix_jobs()
        if not isinstance(job.document.get("name"), str)
    ]
    assert not unnamed, (
        "a matrix job must declare its own name; GitHub otherwise derives one "
        f"from the matrix values, runner label first: {unnamed}"
    )


def test_no_job_name_reads_what_runs_on_reads() -> None:
    """Keep the check name clear of the runner the job resolved.

    Asserted against the job's own ``runs-on`` rather than against the literal
    ``matrix.os``, so renaming the dimension cannot quietly exempt it.
    """
    shared = [
        f"{job.where} name reads {sorted(overlap)}"
        for job in _matrix_jobs()
        if (
            overlap := _references(job.document.get("name"))
            & _references(job.document.get("runs-on"))
        )
    ]
    assert not shared, (
        "a check name must not interpolate what runs-on interpolates; the "
        "required context would then depend on which runner the event "
        f"selected: {shared}"
    )


def test_no_job_name_embeds_a_runner_label() -> None:
    """Refuse a label written into the name rather than interpolated.

    A hard-coded label reads as stable and is not: it either contradicts the
    lane it names or has to change whenever the lane moves.
    """
    embedded = [
        f"{job.where} name names {label!r}"
        for job in _matrix_jobs()
        for label in RUNNER_LABELS
        if label in str(job.document.get("name", ""))
    ]
    assert not embedded, (
        "a check name must not contain a runner label; name the platform "
        f"instead: {embedded}"
    )


def test_matrix_rows_render_distinct_job_names() -> None:
    """Keep one check per lane.

    An explicit name is shared by every row that renders it identically, so a
    name omitting the dimension that separates two lanes collapses their two
    required contexts into one and hides a red lane behind a green one.
    """
    collisions = []
    for job in _matrix_jobs():
        declared = job.document.get("name")
        if not isinstance(declared, str):
            continue
        rendered = [_render(declared, row) for row in _matrix_rows(job.document)]
        if len(set(rendered)) != len(rendered):
            collisions.append(f"{job.where} renders {rendered}")
    assert not collisions, (
        "each matrix row must render its own check name, or two lanes report "
        f"as one context: {collisions}"
    )


@pytest.mark.parametrize(
    ("name", "runs_on", "expected"),
    [
        pytest.param("t (${{ matrix.os }})", "${{ matrix.os }}", True, id="same-key"),
        pytest.param(
            "t (${{ matrix.runner }})", "${{ matrix.runner }}", True, id="renamed-key"
        ),
        pytest.param(
            "t (${{ matrix.platform }})", "${{ matrix.os }}", False, id="platform-word"
        ),
        pytest.param("t (linux)", "${{ matrix.os }}", False, id="no-expression"),
    ],
)
def test_overlap_discriminates(name: str, runs_on: str, *, expected: bool) -> None:
    """Drive the overlap rule directly, in both directions.

    ``ci.yml`` declares one shape, so reading the rule off the workflow would
    pass whether it discriminated or not. The renamed-key case is the one
    that matters: a rule hard-coded to ``matrix.os`` would admit it.
    """
    assert bool(_references(name) & _references(runs_on)) is expected, (
        f"name {name!r} against runs-on {runs_on!r} must "
        f"{'overlap' if expected else 'not overlap'}"
    )
