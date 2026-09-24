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
name shares no expression reference with anything that chooses the runner, it
embeds no runner label, and the names its matrix rows render stay distinct.
The parsing lives in :mod:`job_name_support`.

Run with:

    pytest tests/workflow_contracts/job_name_shape_test.py
"""

import typing as typ

import pytest
from job_name_support import (
    RUNNER_LABELS,
    matrix_jobs,
    matrix_rows,
    references,
    render_job_name,
    runner_references,
)


def test_the_estate_declares_at_least_one_matrix_job() -> None:
    """Refuse a vacuous pass.

    Every contract below is a list comprehension over the matrix jobs, so a
    traversal that found none would report success while asserting nothing.
    """
    assert matrix_jobs(), (
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
        for job in matrix_jobs()
        if not isinstance(job.document.get("name"), str)
    ]
    assert not unnamed, (
        "a matrix job must declare its own name; GitHub otherwise derives one "
        f"from the matrix values, runner label first: {unnamed}"
    )


def test_no_job_name_reads_what_runs_on_reads() -> None:
    """Keep the check name clear of everything that chooses the runner.

    Asserted against the job's own ``runs-on`` rather than against the literal
    ``matrix.os``, so renaming the dimension cannot quietly exempt it, and
    through the matrix values that ``runs-on`` resolves, so a name reading the
    ``fork`` field behind the label is refused as well. Those two share no
    reference with each other, so comparing against ``runs-on`` alone would
    admit the second.
    """
    shared = [
        f"{job.where} name reads {sorted(overlap)}"
        for job in matrix_jobs()
        if (
            overlap := references(job.document.get("name"))
            & runner_references(job.document)
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
        for job in matrix_jobs()
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
    for job in matrix_jobs():
        declared = job.document.get("name")
        if not isinstance(declared, str):
            continue
        rendered = [render_job_name(declared, row) for row in matrix_rows(job.document)]
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
    assert bool(references(name) & references(runs_on)) is expected, (
        f"name {name!r} against runs-on {runs_on!r} must "
        f"{'overlap' if expected else 'not overlap'}"
    )


#: A job shaped like `ci.yml:build-test`: the label is resolved from a matrix
#: key, and the value behind that key branches on the head repository.
_FORK_LANE_JOB: typ.Final[dict[str, object]] = {
    "runs-on": "${{ matrix.os }}",
    "strategy": {
        "matrix": {
            "include": [
                {
                    "os": (
                        "${{ github.event.pull_request.head.repo.fork"
                        " && 'ubuntu-latest' || 'ubicloud-standard-2' }}"
                    ),
                },
                {"os": "windows-latest"},
            ]
        }
    },
}
_FORK_FIELD = "github.event.pull_request.head.repo.fork"


def test_runner_references_reach_through_the_matrix() -> None:
    """Follow ``runs-on`` into the matrix value it resolves.

    ``runs-on`` and the fork field share no reference, so a rule comparing the
    two directly cannot see that the label depends on the head repository.
    """
    assert runner_references(_FORK_LANE_JOB) == {"matrix.os", _FORK_FIELD}, (
        "the references that choose a runner are the ones runs-on reads plus "
        "the ones the matrix values it resolves read"
    )


def test_runner_references_stop_at_a_literal_matrix_value() -> None:
    """Do not invent a dependency a literal label does not have.

    A lane whose matrix value is a plain label depends on nothing, and a rule
    that returned references anyway would refuse names that are perfectly
    stable.
    """
    literal_job: dict[str, object] = {
        "runs-on": "${{ matrix.os }}",
        "strategy": {"matrix": {"include": [{"os": "ubuntu-latest"}]}},
    }

    assert runner_references(literal_job) == {"matrix.os"}, (
        "a literal matrix label reads no context, so it adds no reference"
    )


@pytest.mark.parametrize(
    ("name", "expected"),
    [
        pytest.param("build-test (${{ matrix.platform }})", False, id="platform-word"),
        pytest.param("build-test (${{ matrix.os }})", True, id="matrix-os"),
        pytest.param(
            "build-test (${{ github.event.pull_request.head.repo.fork"
            " && 'fork' || 'internal' }})",
            True,
            id="fork-field-behind-the-label",
        ),
        pytest.param(
            "build-test (${{ github.event.pull_request.head.repo.private"
            " && 'private' || 'public' }})",
            False,
            id="sibling-field-the-label-does-not-read",
        ),
    ],
)
def test_name_overlap_covers_the_matrix_value(name: str, *, expected: bool) -> None:
    """Refuse a name reading the field the label branches on.

    The fork case is the one a rule comparing the name with ``runs-on`` alone
    admits: such a name renders differently on a fork's pull request and on an
    internal one, which is exactly the instability the explicit name exists to
    remove. The sibling ``private`` field proves the rule narrow: it reads
    almost identically and the label does not branch on it, so a name using it
    is stable and must be admitted.
    """
    overlap = references(name) & runner_references(_FORK_LANE_JOB)

    assert bool(overlap) is expected, (
        f"name {name!r} must {'be refused' if expected else 'be admitted'}; "
        f"overlap was {sorted(overlap)}"
    )
