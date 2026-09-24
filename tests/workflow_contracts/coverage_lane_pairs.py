"""Pair each ratcheting merge-gate lane with the trunk job writing its baseline.

The ratchet compares a pull request's coverage against the baseline the trunk
wrote, so each gate lane and its baseline writer must measure the same thing.
The merge gate resolves its inputs from a matrix row and the publisher states
them literally, so the two are compared only after the gate's ``${{ matrix.*
}}`` and ``${{ env.* }}`` references have been resolved against the row the
step actually runs for. Which row that is, is read from the step's own guard
through :func:`guard_conditions.admits`, not typed beside it.

See Also
--------
codescene_coverage_test : The parity contracts built on these pairs.
"""

import dataclasses
import re
import typing as typ

from codescene_coverage_support import (
    PR_WORKFLOW,
    PUBLISHER,
    PUBLISHER_COVERAGE_STEP,
    coverage_step,
)
from guard_conditions import admits
from runner_label_support import runner_label_expression
from workflow_queries import iter_steps
from workflow_support import MissingKeyError, WorkflowShapeError, job

if typ.TYPE_CHECKING:
    import collections.abc as cabc

#: The merge-gate job that runs every coverage lane.
GATE_JOB: typ.Final[str] = "build-test"
#: Runner labels this repository deploys on, mapped to the `runner.os` value
#: GitHub gives them; both arms of the Linux lane's fork fallback are here.
#: Named rather than inferred from the label text: a label is a shape and a
#: provider, and only a table says which operating system it boots.
RUNNER_PLATFORMS: typ.Final[dict[str, str]] = {
    "ubicloud-standard-2": "Linux",
    "ubuntu-latest": "Linux",
    "windows-latest": "Windows",
}
#: Inputs that decide where a report goes rather than what it measures. The
#: gate declines publication and the publisher keeps the default, by design.
PUBLICATION_INPUTS: typ.Final = frozenset({"publish-artefact"})
_EXPRESSION = re.compile(r"\$\{\{\s*(?P<scope>matrix|env)\.(?P<key>[\w-]+)\s*\}\}")


@dataclasses.dataclass(frozen=True, slots=True)
class LanePair:
    """One gate lane and the publisher job that writes the baseline it reads."""

    gate_step: str
    publisher_job: str


#: Every pairing the ratchet relies on. The strict Windows lane has no pair of
#: its own: the baseline cache is keyed by `runner.os` alone, so it reads the
#: default-features baseline, and the developer guide records that residual.
PAIRS: typ.Final = (
    LanePair("Test and Measure Coverage (Linux)", "coverage-upload"),
    LanePair(
        "Test and Measure Coverage (Windows, default features)",
        "coverage-baseline-windows",
    ),
)


class AmbiguousLaneError(WorkflowShapeError):
    """A gate step's guard admits other than exactly one matrix row.

    Parameters
    ----------
    step : str
        The gate step.
    count : int
        How many rows its guard admits on a pull request.
    """

    def __init__(self, step: str, count: int) -> None:
        super().__init__(
            f"{step!r} must run for exactly one matrix row on a pull request; "
            f"its guard admits {count}"
        )


class MixedPlatformLabelError(WorkflowShapeError):
    """A runner declaration can resolve to labels on different platforms.

    Parameters
    ----------
    declared : str
        The declaration.
    platforms : list[str]
        The platforms its labels boot.
    """

    def __init__(self, declared: str, platforms: list[str]) -> None:
        super().__init__(
            f"{declared!r} resolves to runners on {platforms}; a lane must "
            "boot one platform whichever label the event selects"
        )


def labels_of(declared: str) -> frozenset[str]:
    """Return every label a runner declaration can resolve to.

    The Linux lane's label is a two-armed expression, so a pull request from
    a fork reaches a GitHub-hosted runner and every other event reaches
    Ubicloud. Both arms are labels the lane can run on.

    Parameters
    ----------
    declared : str
        A literal label or a two-armed runner expression.

    Returns
    -------
    frozenset[str]
        The label itself, or both arms of the expression.

    Examples
    --------
    >>> sorted(labels_of("windows-latest"))
    ['windows-latest']
    """
    if not declared.lstrip().startswith("${{"):
        return frozenset({declared})
    label = runner_label_expression(declared)
    return frozenset({label.when_true, label.when_false})


def platform_of(declared: str) -> str:
    """Return the `runner.os` a declaration boots, whichever arm is chosen.

    Parameters
    ----------
    declared : str
        A literal label or a two-armed runner expression.

    Returns
    -------
    str
        The platform every label it can resolve to boots.

    Raises
    ------
    MixedPlatformLabelError
        If its labels boot different platforms.
    """
    platforms = sorted({RUNNER_PLATFORMS[label] for label in labels_of(declared)})
    if len(platforms) != 1:
        raise MixedPlatformLabelError(declared, platforms)
    return platforms[0]


def render(value: object) -> str:
    """Render a parsed scalar as the text GitHub passes to an action input.

    Parameters
    ----------
    value : object
        A parsed YAML scalar.

    Returns
    -------
    str
        ``true``/``false`` for booleans, the text otherwise.
    """
    return str(value).lower() if isinstance(value, bool) else str(value)


def job_env(workflow_name: str, job_name: str) -> dict[str, str]:
    """Return one job's ``env`` block, rendered as its steps would read it.

    Parameters
    ----------
    workflow_name : str
        The workflow file name.
    job_name : str
        The job.

    Returns
    -------
    dict[str, str]
        The job-scope variables; empty when the job declares none.
    """
    declared = job(workflow_name, job_name).get("env")
    if not isinstance(declared, dict):
        return {}
    return {str(key): render(value) for key, value in declared.items()}


def matrix_rows() -> list[dict[str, str]]:
    """Return the gate's matrix rows, rendered as action inputs would see them.

    Returns
    -------
    list[dict[str, str]]
        One mapping per ``include`` row.

    Raises
    ------
    MissingKeyError
        If the gate declares no ``strategy.matrix.include`` list.
    """
    strategy = job(PR_WORKFLOW, GATE_JOB).get("strategy")
    matrix = strategy.get("matrix") if isinstance(strategy, dict) else None
    include = matrix.get("include") if isinstance(matrix, dict) else None
    if not isinstance(include, list):
        raise MissingKeyError(f"{PR_WORKFLOW}:{GATE_JOB}", "strategy.matrix.include")
    return [
        {str(key): render(value) for key, value in row.items()}
        for row in include
        if isinstance(row, dict)
    ]


def leg_context(row: cabc.Mapping[str, str], event_name: str) -> dict[str, str]:
    """Return what a gate step's guard reads for one matrix leg and event.

    The ref is `main`, the case where a push is a trunk run; a pull request
    is decided by its event name whatever the ref.

    Parameters
    ----------
    row : cabc.Mapping[str, str]
        A rendered matrix row.
    event_name : str
        The triggering event, such as ``pull_request`` or ``push``.

    Returns
    -------
    dict[str, str]
        The evaluation context.
    """
    return {
        "github.event_name": event_name,
        "github.ref": "refs/heads/main",
        "runner.os": platform_of(row["os"]),
        **{f"matrix.{key}": value for key, value in row.items()},
    }


def gate_row(step_name: str) -> dict[str, str]:
    """Return the one matrix row a gate step runs for on a pull request.

    Parameters
    ----------
    step_name : str
        The gate step.

    Returns
    -------
    dict[str, str]
        The row its guard admits.

    Raises
    ------
    AmbiguousLaneError
        If the guard admits no row or several.
    """
    guard = next(
        str(reference.step.get("if", ""))
        for reference in iter_steps(PR_WORKFLOW)
        if reference.job == GATE_JOB and reference.name == step_name
    )
    admitted = [
        row for row in matrix_rows() if admits(guard, leg_context(row, "pull_request"))
    ]
    if len(admitted) != 1:
        raise AmbiguousLaneError(step_name, len(admitted))
    return admitted[0]


def resolve(value: object, scopes: cabc.Mapping[str, cabc.Mapping[str, str]]) -> str:
    """Substitute ``matrix`` and ``env`` references in one input value.

    A reference neither scope names is left as written: a value a step
    exports to ``GITHUB_ENV`` at run time has no static value, and two copies
    of a step that read it agree exactly when they name it identically.

    Parameters
    ----------
    value : object
        The declared input.
    scopes : cabc.Mapping[str, cabc.Mapping[str, str]]
        ``matrix`` and ``env`` values.

    Returns
    -------
    str
        The input as the action would receive it.

    Examples
    --------
    >>> resolve("a ${{ matrix.f }}", {"matrix": {"f": "b"}, "env": {}})
    'a b'
    """
    return _EXPRESSION.sub(
        lambda found: scopes[found["scope"]].get(found["key"], found[0]),
        render(value),
    )


def measured_inputs(
    inputs: cabc.Mapping[str, object],
    scopes: cabc.Mapping[str, cabc.Mapping[str, str]],
) -> dict[str, str]:
    """Return a step's inputs that select what is measured, resolved.

    Parameters
    ----------
    inputs : cabc.Mapping[str, object]
        The step's ``with`` mapping.
    scopes : cabc.Mapping[str, cabc.Mapping[str, str]]
        ``matrix`` and ``env`` values.

    Returns
    -------
    dict[str, str]
        Every input except the publication ones, resolved.
    """
    return {
        str(key): resolve(value, scopes)
        for key, value in inputs.items()
        if key not in PUBLICATION_INPUTS
    }


def gate_inputs(pair: LanePair) -> dict[str, str]:
    """Return what the gate lane measures, resolved against its own row.

    Parameters
    ----------
    pair : LanePair
        The pairing.

    Returns
    -------
    dict[str, str]
        The resolved selection.
    """
    scopes = {"matrix": gate_row(pair.gate_step), "env": job_env(PR_WORKFLOW, GATE_JOB)}
    return measured_inputs(coverage_step(PR_WORKFLOW, pair.gate_step, GATE_JOB), scopes)


def publisher_inputs(pair: LanePair) -> dict[str, str]:
    """Return what the baseline writer measures.

    Parameters
    ----------
    pair : LanePair
        The pairing.

    Returns
    -------
    dict[str, str]
        The resolved selection.
    """
    scopes = {"matrix": {}, "env": job_env(PUBLISHER, pair.publisher_job)}
    inputs = coverage_step(PUBLISHER, PUBLISHER_COVERAGE_STEP, pair.publisher_job)
    return measured_inputs(inputs, scopes)
