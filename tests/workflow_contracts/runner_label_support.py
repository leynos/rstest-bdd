"""Runner labels: the constants, the parser, and the traversal.

A runner label is either a literal such as ``ubuntu-latest`` or a conditional
expression that resolves one at job setup. The expression form brings two
failures that a green run cannot show, so the contracts need both what a label
says and how it was written. Keeping that here leaves
:mod:`workflow_support` to workflow documents and :mod:`workflow_queries` to
steps.

See Also
--------
runner_label_shape_test : The contracts these helpers serve.
"""

import dataclasses
import re
import typing as typ

from workflow_queries import workflow_names
from workflow_support import (
    GITHUB_HOSTED_LINUX,
    UBICLOUD_LINUX_LABEL,
    WorkflowShapeError,
    jobs,
)

if typ.TYPE_CHECKING:
    import collections.abc as cabc

# A pull request from a fork cannot obtain an Ubicloud runner, so the Linux
# lane resolves its label from the head repository's `fork` field. The field is
# named separately from the whole expression because the narrow mutation this
# contract must refuse swaps it for a sibling field of the same object.
FORK_FIELD = "github.event.pull_request.head.repo.fork"
FORK_FALLBACK_LINUX_LABEL = (
    "${{ github.event.pull_request.head.repo.fork "
    f"&& '{GITHUB_HOSTED_LINUX}' || '{UBICLOUD_LINUX_LABEL}' }}}}"
)
# Collapses the folding whitespace a block scalar may leave in a label.
# Spelled out rather than written `\\s`, which in Python also matches U+001C
# to U+001F; those are not whitespace to a runner label and must not be
# silently absorbed here.
_LABEL_WHITESPACE = re.compile(r"[ \t\n\r]+")
_RUNNER_EXPRESSION = re.compile(
    r"\$\{\{ *(?P<guard>.+?) *&& *'(?P<when_true>[^']*)'"
    r" *\|\| *'(?P<when_false>[^']*)' *\}\}"
)
#: The labels the Linux lane can resolve to. A step condition that names
#: either of them is keyed on the label and not on the runner, whichever way
#: round the comparison is written.
RESOLVED_LINUX_LABELS = (GITHUB_HOSTED_LINUX, UBICLOUD_LINUX_LABEL)
#: Any reference to the resolved label, not just a comparison against one.
#: `matrix.os` is the expression's result, so a condition reading it at all
#: has already stopped meaning what it says on one arm.
_MATRIX_OS_REFERENCE = re.compile(r"\bmatrix\.os\b")


class NotARunnerExpressionError(WorkflowShapeError):
    """A runner label was not a two-armed conditional expression.

    Parameters
    ----------
    raw : str
        The label as declared, quoted in the message so a reader sees the
        folding whitespace as well as the text.
    """

    def __init__(self, raw: str) -> None:
        super().__init__(f"{raw!r} is not a conditional runner label")


@dataclasses.dataclass(frozen=True, slots=True)
class RunnerLabel:
    """One declared runner label, raw and parsed.

    The raw declaration is kept beside the derived guard and arms because the
    two answer different questions. The arms say which runner each event
    reaches; the raw text says whether the declaration survived YAML folding.
    A label whose continuation is indented one level deeper parses to a string
    containing a line break, which GitHub evaluates anyway, so the parse alone
    cannot detect it.

    Attributes
    ----------
    raw : str
        The label exactly as the document declares it.
    guard : str
        The expression's condition, with folding whitespace collapsed.
    when_true : str
        The label used when the guard holds.
    when_false : str
        The label used otherwise.
    """

    raw: str
    guard: str
    when_true: str
    when_false: str

    @property
    def spans_lines(self) -> bool:
        """Whether the declaration parsed to more than one line.

        Returns
        -------
        bool
            True when the raw label contains a line break.
        """
        return "\n" in self.raw


def collapse_label_whitespace(raw: str) -> str:
    r"""Collapse a label's folding whitespace to single spaces.

    A folded scalar leaves a space where it joined two lines, and a
    misplaced continuation leaves a newline and the continuation's
    indent. Contracts that assert what a label *says* read it through
    here, so only the contract that asserts how it was *written* can
    fail on the difference.

    Parameters
    ----------
    raw : str
        The label as declared.

    Returns
    -------
    str
        The label with runs of spaces, tabs and line breaks collapsed to
        one space, and no leading or trailing whitespace.

    Examples
    --------
    >>> collapse_label_whitespace("${{ a\n      && 'x' }}")
    "${{ a && 'x' }}"
    """
    return _LABEL_WHITESPACE.sub(" ", raw).strip()


def runner_label_expression(raw: str) -> RunnerLabel:
    """Parse a two-armed conditional runner label.

    Folding whitespace is collapsed before matching, so a correctly placed
    continuation and a single-line declaration parse identically. Detecting a
    misplaced continuation is :attr:`RunnerLabel.spans_lines`, not this.

    Parameters
    ----------
    raw : str
        The label as declared, for example
        ``"${{ github.event.pull_request.head.repo.fork && 'ubuntu-latest'
        || 'ubicloud-standard-2' }}"``.

    Returns
    -------
    RunnerLabel
        The raw declaration with its guard and both arms.

    Raises
    ------
    NotARunnerExpressionError
        If the label is not a two-armed conditional expression.

    Examples
    --------
    >>> label = runner_label_expression(
    ...     "${{ github.event.pull_request.head.repo.fork"
    ...     " && 'ubuntu-latest' || 'ubicloud-standard-2' }}"
    ... )
    >>> label.guard
    'github.event.pull_request.head.repo.fork'
    >>> label.when_true, label.when_false
    ('ubuntu-latest', 'ubicloud-standard-2')
    """
    collapsed = collapse_label_whitespace(raw)
    match = _RUNNER_EXPRESSION.fullmatch(collapsed)
    if match is None:
        raise NotARunnerExpressionError(raw)
    return RunnerLabel(
        raw=raw,
        guard=match["guard"],
        when_true=match["when_true"],
        when_false=match["when_false"],
    )


def literal_label_guard(condition: str) -> str | None:
    """Say why a step condition is keyed on a runner label, if it is.

    The Linux lane's label is an expression, so ``matrix.os`` is whichever
    arm the event selected. A condition that reads it, or that names either
    label it can resolve to, switches the step off on the arm it did not
    name and fails nothing while doing so. Matching the operator would miss
    ``matrix.os != 'ubuntu-latest'`` and ``'ubicloud-standard-2' ==
    matrix.os``, both of which skip a step on one arm, so the reference
    itself is what is refused.

    Parameters
    ----------
    condition : str
        A step's ``if`` expression, empty when it declares none.

    Returns
    -------
    str | None
        A phrase naming what the condition is keyed on, or None when it is
        keyed on neither the resolved label nor a label literal.

    Examples
    --------
    >>> literal_label_guard("${{ matrix.os == 'ubicloud-standard-2' }}")
    'reads matrix.os'
    >>> literal_label_guard("${{ matrix.os != 'ubuntu-latest' }}")
    'reads matrix.os'
    >>> literal_label_guard("${{ contains(runs.labels, 'ubuntu-latest') }}")
    "names 'ubuntu-latest'"
    >>> literal_label_guard("${{ runner.os == 'Linux' }}") is None
    True
    """
    if _MATRIX_OS_REFERENCE.search(condition):
        return "reads matrix.os"
    named = [label for label in RESOLVED_LINUX_LABELS if label in condition]
    if named:
        return "names " + ", ".join(repr(label) for label in named)
    return None


@dataclasses.dataclass(frozen=True, slots=True)
class RunnerLabelRef:
    """One runner label as declared, with enough context to name it.

    Attributes
    ----------
    workflow : str
        File name of the workflow that declares it.
    job : str
        Key of the declaring job.
    source : str
        Where within the job the label was declared, for example
        ``runs-on`` or ``strategy.matrix.include[0].os``.
    raw : str
        The label exactly as the document declares it.
    """

    workflow: str
    job: str
    source: str
    raw: str

    @property
    def where(self) -> str:
        """A human-readable location for a failure message.

        Returns
        -------
        str
            The workflow, job and declaration site.
        """
        return f"{self.workflow}:{self.job}:{self.source}"


def _runs_on_refs(
    workflow_name: str, job_name: str, job_document: dict[str, object]
) -> cabc.Iterator[RunnerLabelRef]:
    """Yield a job's own ``runs-on`` labels.

    GitHub accepts a single label or a list of them, and a job that calls a
    reusable workflow declares neither.

    Yields
    ------
    RunnerLabelRef
        Each label the job declares under ``runs-on``.
    """
    match job_document.get("runs-on"):
        case str() as declared:
            yield RunnerLabelRef(workflow_name, job_name, "runs-on", declared)
        case list() as declared:
            for position, entry in enumerate(declared):
                if isinstance(entry, str):
                    source = f"runs-on[{position}]"
                    yield RunnerLabelRef(workflow_name, job_name, source, entry)
        case _:
            return


def _matrix_include(job_document: dict[str, object]) -> list[object]:
    """Return a job's matrix include rows, or none when it declares no matrix.

    Parameters
    ----------
    job_document : dict[str, object]
        A job mapping.

    Returns
    -------
    list[object]
        The include rows, empty when the job declares no matrix or the
        matrix declares no include list.
    """
    strategy = job_document.get("strategy")
    matrix = strategy.get("matrix") if isinstance(strategy, dict) else None
    include = matrix.get("include") if isinstance(matrix, dict) else None
    return include if isinstance(include, list) else []


def _declared_os(row: object) -> str | None:
    """Return an include row's ``os`` label when it declares one.

    Parameters
    ----------
    row : object
        One matrix include row, which YAML permits to be anything.

    Returns
    -------
    str | None
        The declared label, or None when the row declares no string ``os``.
    """
    if not isinstance(row, dict):
        return None
    label = row.get("os")
    return label if isinstance(label, str) else None


def _matrix_label_refs(
    workflow_name: str, job_name: str, job_document: dict[str, object]
) -> cabc.Iterator[RunnerLabelRef]:
    """Return the ``os`` labels a job's matrix include rows declare.

    Returns
    -------
    cabc.Iterator[RunnerLabelRef]
        Each label an include row declares, in declaration order.
    """
    labels = (
        (position, _declared_os(row))
        for position, row in enumerate(_matrix_include(job_document))
    )
    return (
        RunnerLabelRef(
            workflow_name,
            job_name,
            f"strategy.matrix.include[{position}].os",
            label,
        )
        for position, label in labels
        if label is not None
    )


def runner_labels() -> cabc.Iterator[RunnerLabelRef]:
    """Yield every runner label the workflow estate declares.

    Covers both the direct ``runs-on`` declaration and the matrix ``os``
    values a ``runs-on: ${{ matrix.os }}`` resolves from, because the folding
    hazard belongs to the declaration and not to the key it is written under.

    Yields
    ------
    RunnerLabelRef
        Each declared label, with its workflow, job and declaration site.
    """
    for name in workflow_names():
        for job_name, job_document in jobs(name).items():
            yield from _runs_on_refs(name, job_name, job_document)
            yield from _matrix_label_refs(name, job_name, job_document)
