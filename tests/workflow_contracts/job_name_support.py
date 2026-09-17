"""Job names: reading them, rendering them, and finding the jobs that need one.

A matrix job's check name is what branch protection requires by, so it has to
be stable across events. GitHub derives one from the matrix values with ``os``
first, which is not stable once a label is an expression, so the jobs declare
their own. Reading a declared name back, and working out what it is allowed to
read, is the parsing these helpers do; the rules built on them are
:mod:`job_name_shape_test`.

See Also
--------
job_name_shape_test : The contracts these helpers serve.
runner_label_support : The same split for the labels themselves.
"""

import re
import typing as typ

from runner_label_support import RESOLVED_LINUX_LABELS
from workflow_queries import workflow_names
from workflow_support import GITHUB_HOSTED_WINDOWS, jobs

#: these is keyed on the runner even when it interpolates nothing.
RUNNER_LABELS = (*RESOLVED_LINUX_LABELS, GITHUB_HOSTED_WINDOWS)
_EXPRESSION = re.compile(r"\$\{\{(?P<body>.*?)\}\}", re.DOTALL)
_REFERENCE = re.compile(r"[A-Za-z_][A-Za-z0-9_-]*(?:\.[A-Za-z_][A-Za-z0-9_-]*)+")
#: Prefix of a reference that reads a matrix value rather than a context.
_MATRIX_PREFIX = "matrix."


def references(text: object) -> set[str]:
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
    >>> sorted(references("${{ matrix.os }}"))
    ['matrix.os']
    >>> references("ubuntu-latest")
    set()
    """
    if not isinstance(text, str):
        return set()
    found: set[str] = set()
    for match in _EXPRESSION.finditer(text):
        found.update(_REFERENCE.findall(match["body"]))
    return found


def render_job_name(name: str, row: dict[str, object]) -> str:
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
    >>> render_job_name("t (${{ matrix.platform }})", {"platform": "linux"})
    't (linux)'
    """

    def substitute(match: re.Match[str]) -> str:
        reference = match["body"].strip()
        key = reference.removeprefix("matrix.")
        return str(row[key]) if key in row else match.group(0)

    return _EXPRESSION.sub(substitute, name)


def matrix_rows(job_document: dict[str, object]) -> list[dict[str, object]]:
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


def runner_references(job_document: dict[str, object]) -> set[str]:
    """Return every reference that can decide which runner a job gets.

    ``runs-on`` usually reads one matrix key and stops there, but the value
    behind that key is itself an expression: this repository's Linux lane
    resolves its label from the head repository's ``fork`` field. A name
    reading that field directly would differ between a fork's pull request
    and an internal one, and comparing the name with ``runs-on`` alone would
    not notice, because the two share no reference in common.

    Parameters
    ----------
    job_document : dict[str, object]
        A job mapping.

    Returns
    -------
    set[str]
        The references ``runs-on`` reads, plus the references read by each
        matrix value it resolves.
    """
    direct = references(job_document.get("runs-on"))
    resolved = set(direct)
    for reference in direct:
        if not reference.startswith(_MATRIX_PREFIX):
            continue
        key = reference.removeprefix(_MATRIX_PREFIX)
        for row in matrix_rows(job_document):
            resolved |= references(row.get(key))
    return resolved


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


def matrix_jobs() -> list[JobRef]:
    """Return every job in the estate that declares a matrix.

    Returns
    -------
    list[JobRef]
        Each matrix job, in workflow and declaration order.
    """
    return [
        JobRef(workflow_name, job_name, job_document)
        for workflow_name in workflow_names()
        for job_name, job_document in jobs(workflow_name).items()
        if matrix_rows(job_document)
    ]
