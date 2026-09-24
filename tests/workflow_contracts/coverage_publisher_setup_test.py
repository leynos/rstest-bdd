"""Hold the publisher's jobs to the merge gate's setup, and own one cache.

`coverage-main.yml` repeats the merge gate's setup rather than sharing it,
because the contracts here read each workflow's steps and a composite action
would hide them. The price of repetition is drift: a tool pin raised in
`ci.yml` and not here, or a cache key computed differently, changes nothing
visible and makes every trunk run restore nothing and compile cold. These
rules state the repetition as an equality instead of trusting the copy.

They also hold the compiler cache to one writer per platform. The merge gate's
trunk run no longer builds the instrumented workspace, so the archive the
pull-request coverage lanes read is written here, by the job that does.

Run via ``make test-workflow-contracts``.
"""

import typing as typ

import pytest
from codescene_coverage_support import PR_WORKFLOW, PUBLISHER, PUBLISHER_COVERAGE_STEP
from coverage_lane_pairs import (
    GATE_JOB,
    PAIRS,
    gate_row,
    job_env,
    platform_of,
    resolve,
)
from guard_conditions import admits
from workflow_queries import iter_steps
from workflow_support import job, steps

if typ.TYPE_CHECKING:
    import collections.abc as cabc

#: Job-scope variables the publisher may declare beyond the gate's: the
#: toolchain the gate takes from its matrix, and the credential only the
#: upload job holds.
PUBLISHER_ONLY_ENV = frozenset({"RUST_TOOLCHAIN", "CS_ACCESS_TOKEN"})
#: Setup every publisher job must repeat. Named, so deleting a step is a
#: failure rather than one fewer comparison.
REQUIRED_SETUP = frozenset({
    "Checkout",
    "Configure runner parallelism and compiler cache",
    "Compute cache keys",
    "Restore Cargo registry",
    "Restore CI tool binaries",
    "Restore compiler cache",
    "Setup Rust",
    "Install prebuilt sccache",
})
#: Keys that legitimately differ between a gate step and its publisher copy:
#: the gate's names carry a platform suffix, and its guards choose among
#: matrix legs that a single-platform job does not have.
IGNORED_KEYS = frozenset({"name", "if"})
SCCACHE_KEY = "${{ env.SCCACHE_CACHE_KEY }}"
PUBLISH_SCCACHE_KEY = "${{ env.SCCACHE_PUBLISH_CACHE_KEY }}"
#: Each compiler-cache family and the one save step per platform allowed to
#: write it. The instrumented family belongs to the publisher's jobs; the
#: publish dry run's belongs to the gate, the one job that runs that build on
#: the trunk (rstest-bdd#803).
FAMILY_WRITERS = {
    SCCACHE_KEY: sorted((PUBLISHER, pair.publisher_job) for pair in PAIRS),
    PUBLISH_SCCACHE_KEY: [(PR_WORKFLOW, GATE_JOB)],
}


def _test_id(value: object) -> str:
    """Name a matrix row by its runner label, and anything else as itself."""
    return platform_of(value["os"]) if isinstance(value, dict) else str(value)


def _scopes(workflow_name: str, job_name: str, row: cabc.Mapping[str, str]) -> dict:
    """Return the ``matrix`` and ``env`` values one job resolves against."""
    return {"matrix": dict(row), "env": job_env(workflow_name, job_name)}


def _normalized(value: object, scopes: dict) -> object:
    """Resolve references and drop comment-only lines from shell bodies.

    A comment inside a ``run:`` block is part of the parsed text, and the two
    copies explain themselves differently; what must agree is the script.

    Returns
    -------
    object
        The value with names and guards dropped and references resolved.
    """
    match value:
        case dict():
            return {
                str(key): _normalized(entry, scopes)
                for key, entry in value.items()
                if key not in IGNORED_KEYS
            }
        case list():
            return [_normalized(entry, scopes) for entry in value]
        case _:
            text = resolve(value, scopes)
            return "\n".join(
                line
                for line in text.splitlines()
                if not line.lstrip().startswith("# ") and line.strip() != "#"
            )


def _publisher_jobs() -> list[tuple[str, dict[str, str]]]:
    """Return each publisher job with the gate row it mirrors."""
    return [(pair.publisher_job, gate_row(pair.gate_step)) for pair in PAIRS]


def _counterpart(name: str, platform: str) -> dict[str, object] | None:
    """Return the gate step a publisher step copies, if the gate has one."""
    gate_steps = {
        str(step.get("name")): step for step in steps(job(PR_WORKFLOW, GATE_JOB))
    }
    return gate_steps.get(name) or gate_steps.get(f"{name} ({platform})")


def _shared_steps() -> list[tuple[str, str, dict[str, str]]]:
    """Return every publisher step that repeats a gate step."""
    return [
        (job_name, str(step.get("name")), row)
        for job_name, row in _publisher_jobs()
        for step in steps(job(PUBLISHER, job_name))
        if step.get("name") != PUBLISHER_COVERAGE_STEP
        and _counterpart(str(step.get("name")), platform_of(row["os"]))
    ]


@pytest.mark.parametrize(("job_name", "row"), _publisher_jobs(), ids=_test_id)
def test_each_publisher_job_declares_the_gates_pins(
    job_name: str, row: dict[str, str]
) -> None:
    """Carry every job-scope pin the gate carries, at the gate's value.

    Every tool pin feeds a cache key, so a publisher pin one version behind
    the gate's restores a different archive, or none.
    """
    gate = _scopes(PR_WORKFLOW, GATE_JOB, row)["env"]
    publisher = _scopes(PUBLISHER, job_name, {})["env"]

    missing = sorted(set(gate) - set(publisher))
    differing = {
        k: (gate[k], publisher[k])
        for k in set(gate) & set(publisher)
        if gate[k] != publisher[k]
    }
    extra = sorted(set(publisher) - set(gate) - PUBLISHER_ONLY_ENV)
    assert not missing, f"{PUBLISHER}:{job_name} lacks the gate's {missing}"
    assert not differing, f"{PUBLISHER}:{job_name} differs from the gate: {differing}"
    assert not extra, f"{PUBLISHER}:{job_name} declares more than the gate: {extra}"


@pytest.mark.parametrize(("job_name", "row"), _publisher_jobs(), ids=_test_id)
def test_each_publisher_job_repeats_the_required_setup(
    job_name: str, row: dict[str, str]
) -> None:
    """Refuse a publisher job that dropped a setup step rather than drifted."""
    declared = {str(step.get("name")) for step in steps(job(PUBLISHER, job_name))}

    assert declared >= REQUIRED_SETUP, (
        f"{PUBLISHER}:{job_name} must repeat {sorted(REQUIRED_SETUP - declared)}"
    )


@pytest.mark.parametrize(
    ("job_name", "step_name", "row"), _shared_steps(), ids=_test_id
)
def test_each_repeated_step_matches_the_gate(
    job_name: str, step_name: str, row: dict[str, str]
) -> None:
    """Hold every repeated step equal to the gate's, references resolved.

    The gate resolves `${{ matrix.* }}` against the leg this job mirrors, and
    the publisher states the same values in its job env, so both are
    resolved before comparison. Names and guards are excluded: the gate
    chooses among matrix legs and this job has one platform.
    """
    platform = platform_of(row["os"])
    publisher_step = next(
        step
        for step in steps(job(PUBLISHER, job_name))
        if step.get("name") == step_name
    )
    gate = _normalized(
        _counterpart(step_name, platform), _scopes(PR_WORKFLOW, GATE_JOB, row)
    )
    publisher = _normalized(publisher_step, _scopes(PUBLISHER, job_name, {}))

    assert publisher == gate, (
        f"{PUBLISHER}:{job_name}:{step_name!r} has drifted from "
        f"{PR_WORKFLOW}:{GATE_JOB}; gate {gate}, publisher {publisher}"
    )


@pytest.mark.parametrize(
    ("key", "expected"), FAMILY_WRITERS.items(), ids=["instrumented", "publish"]
)
def test_each_compiler_cache_family_has_one_writer_per_platform(
    key: str, expected: list[tuple[str, str]]
) -> None:
    """Write each compiler-cache family from the job that builds what it holds.

    Every key carries `runner.os`, so one save step per platform is one writer
    per key; a second would race it for the reservation, and the loser's
    objects would be lost. The gate's one save step for the publish family
    covers a single platform, which its guard selects.
    """
    writers = sorted(
        (reference.workflow, reference.job)
        for reference in iter_steps()
        if str(reference.uses).startswith("actions/cache/save@")
        and isinstance(inputs := reference.step.get("with"), dict)
        and inputs.get("key") == key
    )

    assert writers == expected, (
        f"the compiler-cache family {key} must be written by {expected} alone; "
        f"it is written by {writers}"
    )


@pytest.mark.parametrize(
    ("event", "ref", "hit", "expected"),
    [
        ("push", "refs/heads/main", "", True),
        ("push", "refs/heads/main", "true", False),
        ("workflow_dispatch", "refs/heads/feature", "", False),
        ("workflow_dispatch", "refs/heads/main", "", False),
    ],
    ids=["trunk miss", "trunk hit", "dispatch on a branch", "dispatch on main"],
)
def test_the_compiler_cache_is_written_only_from_a_trunk_push(
    event: str, ref: str, hit: str, expected: object
) -> None:
    """Evaluate each writer's guard for every way the publisher can start.

    A dispatch, from any ref, measures without persisting anything, which is
    what lets it be run to check the lane without moving what pull requests
    read.
    """
    context = {
        "github.event_name": event,
        "github.ref": ref,
        "steps.sccache-linux.outputs.cache-hit": hit,
        "steps.sccache-windows.outputs.cache-hit": hit,
        "vars.RSTEST_BDD_SCCACHE_LOCAL": "true",
    }
    guards = {
        reference.job: str(reference.step.get("if", ""))
        for reference in iter_steps(PUBLISHER)
        if str(reference.uses).startswith("actions/cache/save@")
    }

    assert guards, f"{PUBLISHER} must write the compiler cache"
    wrong = sorted(
        name for name, guard in guards.items() if admits(guard, context) is not expected
    )
    assert not wrong, (
        f"on a {event} of {ref} with cache-hit={hit!r} these writers "
        f"{'must' if expected else 'must not'} save: {wrong}"
    )
