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

REPO_ROOT = Path(__file__).resolve().parents[2]
MAKEFILE = REPO_ROOT / "Makefile"

TARGET = "prepare-nested-fixtures"
RUNNER = "nextest-run"

#: The filter the nextest override budgets, as nextest receives it.
PREPARATION_FILTER = "test(/nested_fixture_preparation$/)"

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


def test_the_target_runs_the_preparation_tests_under_nextest() -> None:
    """The target runs nextest, not ``cargo test``."""
    lines = recipe(TARGET)

    assert any("nextest run" in line for line in lines), (
        f"{TARGET} should run the preparation tests under nextest, got {lines!r}"
    )


def test_the_target_selects_exactly_the_preparation_tests() -> None:
    """The target's filter is the one the nextest override budgets.

    The filter is asserted in its Makefile spelling, ``$$`` included. ``$$`` is
    what Make requires to pass a literal ``$`` through to the command, so
    comparing the raw text pins the escaping too: a bare ``$`` would be
    expanded by Make before nextest ever saw it, and the filter would mean
    something else.
    """
    expected = PREPARATION_FILTER.replace("$", "$$")
    filters = [
        line.split("-E '", 1)[1].split("'", 1)[0]
        for line in recipe(TARGET)
        if "-E '" in line
    ]

    assert filters == [expected], (
        f"{TARGET} must select the tests the nextest override budgets, or it"
        f" prepares nothing and the timed suites refuse their gate; got {filters!r}"
    )


def test_the_target_uses_the_same_build_environment_as_the_suite() -> None:
    """The stamps record the build environment, so it has to match.

    A preparation run under a different ``RUSTFLAGS`` or feature selection
    warms a tree the timed tests will not reuse: the fingerprint would differ
    and every gate would refuse the stamp the target had just written.
    """
    lines = recipe(TARGET)

    assert any("RUSTFLAGS=" in line for line in lines), (
        f"{TARGET} should pin RUSTFLAGS, as the test target's main leg does,"
        f" got {lines!r}"
    )
    assert any("$(RUST_FLAGS)" in line for line in lines), (
        f"{TARGET} should use the shared RUST_FLAGS, got {lines!r}"
    )


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


def test_the_run_half_is_phony() -> None:
    """The run target names a command, not a file, so it must be phony."""
    phony = [
        line
        for line in MAKEFILE.read_text(encoding="utf-8").splitlines()
        if line.startswith(".PHONY:")
    ]

    assert any(RUNNER in line for line in phony), (
        f"{RUNNER} should be .PHONY, got {phony!r}"
    )


def test_the_target_is_phony() -> None:
    """The target names a command, not a file, so it must be declared phony."""
    phony = [
        line
        for line in MAKEFILE.read_text(encoding="utf-8").splitlines()
        if line.startswith(".PHONY:")
    ]

    assert any(TARGET in line for line in phony), (
        f"{TARGET} should be .PHONY, got {phony!r}"
    )
