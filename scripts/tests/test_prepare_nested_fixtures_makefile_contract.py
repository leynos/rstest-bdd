"""Contract tests for the ``prepare-nested-fixtures`` Makefile target.

The target and the ``nested_fixture_preparation`` override in
``.config/nextest.toml`` are two halves of one policy. The override gives those
tests a 30-minute budget; the target is what selects and runs them before a
filtered suite. Only the override was pinned, by
``crates/rstest-bdd/tests/nextest_config.rs``.

The gap that leaves is not cosmetic. ``cargo nextest run -E <filter>`` exits
zero when the filter matches nothing, so a target whose filter drifted — a
renamed test, a lost ``$$``, an anchor that no longer matches — still prints a
successful run. It would simply have prepared nothing, and every timed test
behind it would then meet the fail-fast gate the whole split exists to satisfy.

The *run* half has its own contract, and it is the one that failed first in
practice. The stamp fingerprints ``RUSTFLAGS``, which ``prepare-nested-fixtures``
pins; a hand-typed ``cargo nextest run`` inherits it from the caller, so the two
commands compute different fingerprints and the run refuses the stamp the
preparation just wrote. Both halves are then correct in isolation and fail to
compose, so ``nextest-run`` is pinned here as the only supported run half: it
must carry the same pinned ``RUSTFLAGS``, must forward ``EXPR`` to nextest
unevaluated, and must refuse to run without one.
"""

from pathlib import Path

import pytest

REPO_ROOT = Path(__file__).resolve().parents[2]
MAKEFILE = REPO_ROOT / "Makefile"

TARGET = "prepare-nested-fixtures"
RUNNER = "nextest-run"

#: The target that runs the end-to-end workflow driver.
DRIVER = "check-prepared-fixture-workflow"

#: The environment assignment both halves of the policy must share.
PINNED_FLAGS = 'RUSTFLAGS="$(RUST_FLAGS) $(STANDARD_RUSTFLAGS)"'


def recipe(target: str) -> list[str]:
    """Return the recipe lines of *target* in the repository Makefile.

    Parameters
    ----------
    target : str
        The Makefile target whose recipe lines are returned.

    Returns
    -------
    list[str]
        The tab-indented recipe lines, without their leading tab. An absent or
        duplicated *target* raises an ``AssertionError`` naming it rather than
        returning.
    """
    lines = MAKEFILE.read_text(encoding="utf-8").splitlines()
    starts = [
        index for index, line in enumerate(lines) if line.startswith(f"{target}:")
    ]
    assert len(starts) == 1, f"expected exactly one {target} target, got {starts!r}"
    start = starts[0] + 1
    end = start
    while end < len(lines) and lines[end].startswith("\t"):
        end += 1
    return [line.removeprefix("\t") for line in lines[start:end]]


def test_the_run_half_pins_the_same_build_environment() -> None:
    """Both halves must stamp and read the *same* fingerprint.

    This is the defect that broke the workflow the first time it ran end to
    end: the preparation pinned ``RUSTFLAGS``, the documented run command did
    not, and step 3 reproduced step 1's refusal verbatim. Asserting the exact
    assignment rather than the presence of ``RUSTFLAGS=`` is what keeps a
    later edit from pinning a *different* one.
    """
    lines = recipe(RUNNER)

    assert any(PINNED_FLAGS in line for line in lines), (
        f"{RUNNER} must use the same pinned build environment as {TARGET}, or"
        f" it refuses the stamp the preparation just wrote; got {lines!r}"
    )


def test_the_run_half_forwards_the_filter_expression_unevaluated() -> None:
    """``EXPR`` is nextest's to parse, so Make must not rewrite it.

    A ``$(EXPR)`` in ``-E`` is what carries the caller's expression through.
    An expanded or re-quoted literal here would silently reduce the selection,
    and an empty one selects everything — the opposite of a filtered run.
    """
    lines = recipe(RUNNER)

    assert any("-E '$(EXPR)'" in line for line in lines), (
        f"{RUNNER} must pass EXPR to nextest verbatim, got {lines!r}"
    )


def test_the_run_half_refuses_an_absent_filter_expression() -> None:
    """An unset ``EXPR`` must stop the target, not run the whole suite.

    ``-E ''`` is an empty filter, which nextest reads as *everything*. A run
    that silently widened from one test to the suite would report success while
    measuring nothing the caller asked for, so the guard is part of the
    contract rather than a convenience.
    """
    lines = recipe(RUNNER)

    assert any("$(if $(EXPR)" in line for line in lines), (
        f"{RUNNER} must refuse an absent EXPR, got {lines!r}"
    )


def phony_lines() -> list[str]:
    """Return the Makefile's ``.PHONY`` declaration lines.

    Returns
    -------
    list[str]
        Every line beginning with ``.PHONY:``, unparsed. Membership is tested
        with ``in`` because one declaration may name several targets.
    """
    return [
        line
        for line in MAKEFILE.read_text(encoding="utf-8").splitlines()
        if line.startswith(".PHONY:")
    ]


@pytest.mark.parametrize("target", [TARGET, RUNNER, DRIVER])
def test_every_target_is_phony(target: str) -> None:
    """Each target names a command, not a file, so each must be phony."""
    phony = phony_lines()

    assert any(target in line for line in phony), (
        f"{target} should be .PHONY, got {phony!r}"
    )


def test_the_workflow_driver_is_reachable_from_make() -> None:
    """The end-to-end driver must be invocable, not merely present.

    Every sibling ``scripts/check_*.py`` is reachable from a Makefile target,
    and this one began as an orphan: a Python file the suite could only run by
    naming its path. A driver nothing invokes is not automated coverage, which
    is the whole reason it exists — it answers a pre-merge row asking for a test
    that runs the documented workflow across process boundaries.
    """
    lines = recipe(DRIVER)
    script = "scripts/check_prepared_fixture_workflow.py"

    assert any(script in line for line in lines), (
        f"{DRIVER} must run {script}, got {lines!r}"
    )
