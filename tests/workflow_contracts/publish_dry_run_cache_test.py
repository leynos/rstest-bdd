"""Hold the publish dry run's compiler-cache family to one writer of its own.

Two archive families share the sccache directory. The instrumented family is
written by ``coverage-main.yml``, the job that builds that shape on the trunk;
the publish family is written by ``ci.yml``'s Windows default-features lane,
the one job that runs ``make publish-check`` on a push to ``main``. A
pull-request Windows lane restores both into the one directory, so one sccache
server reads both, and ``runner_cache_test`` records that directory as the one
path two cache owners may share (rstest-bdd#803).

That exception is safe only while each archive holds its own family's objects.
A writer whose run had also restored the other family would save a copy of it
under its own key, and every pull request would then download the other
family's objects twice. These contracts evaluate each step's guard through
:func:`guard_conditions.admits` for the runs that write, rather than searching
the guard for a phrase, and run the key scripts themselves to show neither
family's ``restore-keys`` prefix can fall back to the other's archive.

Run via ``make test-workflow-contracts``.
"""

import dataclasses
import itertools
import re
import subprocess  # ruff: ignore[suspicious-subprocess-import] - runs a script this repository declares.
import typing as typ

import pytest
from codescene_coverage_support import PR_WORKFLOW, PUBLISHER
from coverage_lane_pairs import (
    GATE_JOB,
    RUNNER_PLATFORMS,
    job_env,
    leg_context,
    matrix_rows,
)
from guard_conditions import admits
from workflow_queries import BASH, StepRef, iter_steps
from workflow_support import SCCACHE_DIRECTORY, job, step_index, steps

if typ.TYPE_CHECKING:
    import collections.abc as cabc
    from pathlib import Path

INSTRUMENTED_KEY: typ.Final = "${{ env.SCCACHE_CACHE_KEY }}"
PUBLISH_KEY: typ.Final = "${{ env.SCCACHE_PUBLISH_CACHE_KEY }}"
WRITER_STEP: typ.Final = "Save publish dry-run compiler cache"
DRY_RUN_STEP: typ.Final = "Publish dry run"
#: The steps that compute the two families' keys, in the order they run.
KEY_STEPS: typ.Final = ("Compute cache keys", "Compute publish dry-run cache key")
#: Every restore output a compiler-cache guard may read.
HIT_OUTPUTS: typ.Final = tuple(
    f"steps.{step_id}.outputs.cache-hit"
    for step_id in ("sccache-linux", "sccache-windows", "sccache-publish")
)
#: Every run that writes a compiler-cache archive, and the family it writes.
EXPECTED_WRITERS: typ.Final = frozenset({
    (PR_WORKFLOW, GATE_JOB, PUBLISH_KEY),
    (PUBLISHER, "coverage-upload", INSTRUMENTED_KEY),
    (PUBLISHER, "coverage-baseline-windows", INSTRUMENTED_KEY),
})
_ENV_REFERENCE = re.compile(r"^\$\{\{\s*env\.(?P<name>\w+)\s*\}\}$")


@dataclasses.dataclass(frozen=True, slots=True)
class Start:
    """How one run starts: its event, ref, restore result, and cache mode."""

    event: str
    ref: str = "refs/heads/main"
    hit: str = ""
    local: str = ""


def _context(row: cabc.Mapping[str, str], start: Start) -> dict[str, str]:
    """Return every value a compiler-cache guard reads for one run."""
    return {
        **leg_context(row, start.event),
        "github.ref": start.ref,
        "vars.RSTEST_BDD_SCCACHE_LOCAL": start.local,
        **dict.fromkeys(HIT_OUTPUTS, start.hit),
    }


def _directory_steps(workflow_name: str, job_name: str, verb: str) -> list[StepRef]:
    """Return one job's restore or save steps on the sccache directory."""
    return [
        reference
        for reference in iter_steps(workflow_name)
        if reference.job == job_name
        and reference.uses.startswith(f"actions/cache/{verb}@")
        and isinstance(inputs := reference.step.get("with"), dict)
        and str(inputs.get("path", "")).strip() == SCCACHE_DIRECTORY
    ]


def _input(reference: StepRef, name: str) -> str:
    """Return one of a cache step's inputs, or the empty string."""
    inputs = reference.step.get("with")
    return str(inputs.get(name, "")) if isinstance(inputs, dict) else ""


def _key(reference: StepRef) -> str:
    """Return the key expression a cache step names."""
    return _input(reference, "key")


def _runs(workflow_name: str, job_name: str) -> list[dict[str, str]]:
    """Return a context for every way a job can start on ``main``.

    The gate is expanded over its matrix; a publisher job has one platform,
    read from its runner label.

    Returns
    -------
    list[dict[str, str]]
        One evaluation context per leg, event, and fallback-mode setting.
    """
    if workflow_name == PR_WORKFLOW:
        rows = matrix_rows()
    else:
        rows = [{"os": str(job(workflow_name, job_name).get("runs-on"))}]
    return [
        _context(row, Start(event, local=local))
        for row, event, local in itertools.product(
            rows, ("push", "pull_request", "workflow_dispatch"), ("", "true")
        )
    ]


def _writer_runs() -> list[tuple[str, str, str, dict[str, str]]]:
    """Return each directory save with every run context that admits it."""
    jobs = [(PR_WORKFLOW, GATE_JOB)] + [
        (PUBLISHER, name) for name in ("coverage-upload", "coverage-baseline-windows")
    ]
    return [
        (workflow_name, job_name, _key(save), context)
        for workflow_name, job_name in jobs
        for save in _directory_steps(workflow_name, job_name, "save")
        for context in _runs(workflow_name, job_name)
        if admits(str(save.step.get("if", "")), context)
    ]


def _admitted_restore_keys(
    workflow_name: str, job_name: str, context: cabc.Mapping[str, str]
) -> set[str]:
    """Return the key of every directory restore that runs in ``context``."""
    return {
        _key(restore)
        for restore in _directory_steps(workflow_name, job_name, "restore")
        if admits(str(restore.step.get("if", "")), context)
    }


def _row_name(row: cabc.Mapping[str, str]) -> str:
    """Name a gate row by its platform and feature selection."""
    return f"{RUNNER_PLATFORMS[row['os']]}:{row.get('features') or 'default'}"


@pytest.mark.parametrize(
    ("event", "ref", "hit", "expected"),
    [
        ("push", "refs/heads/main", "", ["Windows:default"]),
        ("push", "refs/heads/main", "true", []),
        ("push", "refs/heads/feature", "", []),
        ("pull_request", "refs/pull/1/merge", "", []),
        ("workflow_dispatch", "refs/heads/main", "", []),
    ],
    ids=["trunk miss", "trunk hit", "branch push", "pull request", "dispatch"],
)
def test_the_publish_family_is_written_by_one_trunk_leg(
    event: str, ref: str, hit: str, expected: list[str]
) -> None:
    """Admit the writer on the Windows default-features leg's trunk miss alone.

    The strict leg runs the same dry run, so a second leg saving the same key
    would race the first for the reservation; a hit has nothing new to save.
    """
    gate_steps = steps(job(PR_WORKFLOW, GATE_JOB))
    guard = str(gate_steps[step_index(gate_steps, WRITER_STEP)].get("if", ""))
    admitted = [
        _row_name(row)
        for row in matrix_rows()
        for local in ("", "true")
        if admits(guard, _context(row, Start(event, ref, hit, local)))
    ]

    assert sorted(set(admitted)) == expected, (
        f"on a {event} of {ref} with cache-hit={hit!r} the publish family must "
        f"be saved by {expected or 'no leg'}; the guard admits {admitted}"
    )


def test_the_publish_family_is_saved_after_the_dry_run() -> None:
    """Save the directory only once the dry run has compiled into it."""
    gate_steps = steps(job(PR_WORKFLOW, GATE_JOB))

    assert step_index(gate_steps, WRITER_STEP) > step_index(gate_steps, DRY_RUN_STEP), (
        f"{WRITER_STEP!r} must follow {DRY_RUN_STEP!r}, or it archives a "
        "directory the dry run has not written yet"
    )


def test_every_writer_run_restores_only_the_family_it_writes() -> None:
    """Keep each archive to its own family's objects.

    In every run whose guard lets a directory archive be saved, the restores
    into that directory that also run must all name the key being saved, and
    at least one must: the writer extends its own family through the
    ``restore-keys`` prefix and must never carry the other family along.
    """
    writer_runs = _writer_runs()
    found = {(workflow, job_name, key) for workflow, job_name, key, _ in writer_runs}
    assert found == EXPECTED_WRITERS, (
        f"the compiler-cache writers are {sorted(found)}, not "
        f"{sorted(EXPECTED_WRITERS)}"
    )
    mixed = [
        f"{workflow}:{job_name} saves {key} after restoring {sorted(restored)} "
        f"on {context['github.event_name']} ({context['runner.os']})"
        for workflow, job_name, key, context in writer_runs
        if (restored := _admitted_restore_keys(workflow, job_name, context)) != {key}
    ]
    assert not mixed, f"each writer must restore its own family alone: {mixed}"


@pytest.mark.parametrize(
    "row",
    [row for row in matrix_rows() if RUNNER_PLATFORMS[row["os"]] == "Windows"],
    ids=_row_name,
)
def test_each_windows_pull_request_leg_restores_both_families(
    row: dict[str, str],
) -> None:
    """Warm the coverage build and the dry run on both Windows legs."""
    context = _context(row, Start("pull_request", ref="refs/pull/1/merge"))

    restored = _admitted_restore_keys(PR_WORKFLOW, GATE_JOB, context)

    assert restored == {INSTRUMENTED_KEY, PUBLISH_KEY}, (
        f"the {_row_name(row)} pull-request leg restores {sorted(restored)}"
    )


@pytest.mark.parametrize(
    "row",
    [row for row in matrix_rows() if RUNNER_PLATFORMS[row["os"]] == "Linux"],
    ids=_row_name,
)
def test_no_linux_leg_restores_the_publish_family(row: dict[str, str]) -> None:
    """Keep the Windows-only family off the Linux legs in every mode.

    Only a Windows leg computes the family's key, so a Linux restore would
    name an unset key; and the Linux dry run has its own backend.
    """
    restoring = [
        f"{event} with RSTEST_BDD_SCCACHE_LOCAL={local!r}"
        for event, local in itertools.product(
            ("push", "pull_request", "workflow_dispatch"), ("", "true")
        )
        if PUBLISH_KEY
        in _admitted_restore_keys(
            PR_WORKFLOW,
            GATE_JOB,
            _context(row, Start(event, local=local)),
        )
    ]

    assert not restoring, f"the {_row_name(row)} leg restores it on {restoring}"


def _rendered_keys(tmp_path: Path) -> dict[str, str]:
    """Run both key scripts as a Windows runner would and read what they export.

    Returns
    -------
    dict[str, str]
        Each exported name and its value.
    """
    gate_steps = steps(job(PR_WORKFLOW, GATE_JOB))
    github_env = tmp_path / "github-env"
    github_env.touch()
    environment = {
        **job_env(PR_WORKFLOW, GATE_JOB),
        "GITHUB_ENV": str(github_env),
        "RUNNER_OS": "Windows",
        "RUNNER_ARCH": "X64",
        "RUNNER_ENVIRONMENT_NAME": "github-hosted",
        "TOOLCHAIN": "stable-x86_64-pc-windows-msvc",
        "LOCKFILE_HASH": "0123abcd",
        "DYLINT_HASH": "4567ef",
    }
    for name in KEY_STEPS:
        script = tmp_path / "keys.sh"
        script.write_text(str(gate_steps[step_index(gate_steps, name)]["run"]))
        subprocess.run(  # ruff: ignore[subprocess-without-shell-equals-true] - the script is this repository's own.
            [BASH, str(script)], check=True, capture_output=True, env=environment
        )
    return dict(
        line.split("=", 1)
        for line in github_env.read_text().splitlines()
        if "=" in line
    )


def test_neither_family_can_fall_back_to_the_others_archive(tmp_path: Path) -> None:
    """Keep each restore step's prefix inside its own family.

    ``restore-keys`` falls back to the newest archive whose key starts with
    the prefix, so a prefix that also matched the other family would restore
    that family's archive into a writer's run with no guard involved.
    """
    rendered = _rendered_keys(tmp_path)
    keys = {
        INSTRUMENTED_KEY: rendered["SCCACHE_CACHE_KEY"],
        PUBLISH_KEY: rendered["SCCACHE_PUBLISH_CACHE_KEY"],
    }
    restores = _directory_steps(PR_WORKFLOW, GATE_JOB, "restore")
    assert restores, f"{PR_WORKFLOW}:{GATE_JOB} must restore the directory"

    for restore in restores:
        match = _ENV_REFERENCE.fullmatch(_input(restore, "restore-keys").strip())
        assert match, f"{restore} must name its restore-keys prefix by variable"
        prefix = rendered[match["name"]]
        matched = sorted(
            family for family, key in keys.items() if key.startswith(prefix)
        )
        assert matched == [_key(restore)], (
            f"{restore}'s prefix {prefix!r} must match its own family "
            f"{_key(restore)} and nothing else; it matches {matched}"
        )
