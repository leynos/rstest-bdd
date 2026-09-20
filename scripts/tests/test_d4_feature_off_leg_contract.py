"""Contract tests for the D4 feature-off leg inside ``make test``.

Constraint 4 of ``docs/execplans/13-1-1-add-parser-neutral-types.md`` requires
the parser-neutral runner's step sequence to be total and ordered, and requires
it to hold identically with the ``diagnostics`` feature on and off. That is a
deliberate divergence from the generated loop, which computes bypassed steps
only under ``diagnostics_enabled()``. The leg is therefore a correctness gate,
not a convenience, and it has two ways to stop being one:

- the line can be dropped, leaving the divergence untested by every gate; or
- the line can keep its text while losing its meaning, which is what happened
  first — it reused ``$(CARGO_FLAGS)``, whose ``--all-features`` cancels
  ``--no-default-features``, so the leg re-enabled ``diagnostics`` and ran the
  same 2055 tests as the leg above it.

The second is the dangerous one, because it is green. These tests are static by
design: they read the Makefile rather than invoking it, so they cost nothing
and cannot themselves be vacuous. Whether the leg actually builds the intended
configuration is evidenced separately, by running it.
"""

from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[2]
MAKEFILE = REPO_ROOT / "Makefile"
PACKAGE_SELECTION = "-p rstest-bdd"
FEATURE_OFF = "--no-default-features"


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
    starts = [i for i, line in enumerate(lines) if line.startswith(f"{target}:")]
    assert len(starts) == 1, f"expected exactly one {target} target, got {starts!r}"
    start = starts[0] + 1
    end = start
    while end < len(lines) and lines[end].startswith("\t"):
        end += 1
    # Comment lines inside a recipe carry a tab too, and a commented-out leg is
    # exactly the regression this file exists to catch, so they are dropped
    # before any assertion sees them.
    return [
        line.removeprefix("\t")
        for line in lines[start:end]
        if not line.removeprefix("\t").startswith("#")
    ]


def feature_off_lines() -> list[str]:
    """Return the ``make test`` recipe lines that build with the feature off.

    The leg is one ``if``/``else`` pair — a nextest branch and a plain
    ``cargo test`` fallback for a machine without nextest — so one leg is one
    or two lines, and both must be checked.

    Returns
    -------
    list[str]
        The recipe lines that carry the feature-off flag; empty when the leg
        is absent, which is the regression the caller asserts against.
    """
    return [line for line in recipe("test") if FEATURE_OFF in line]


def test_make_test_builds_rstest_bdd_without_default_features() -> None:
    """The leg exists, selects the right package, and is not duplicated."""
    lines = feature_off_lines()

    assert 1 <= len(lines) <= 2, (
        f"`make test` should carry exactly one {FEATURE_OFF} leg (a nextest "
        f"branch and its fallback), got {lines!r}"
    )
    for line in lines:
        assert PACKAGE_SELECTION in line, (
            f"the leg must select the crate whose feature it varies, got {line!r}"
        )


def test_feature_off_leg_does_not_reuse_the_shared_flag_variable() -> None:
    """The regression: ``$(CARGO_FLAGS)`` re-enables the feature being varied.

    ``CARGO_FLAGS`` is ``--workspace --all-targets --all-features``, and a later
    ``--all-features`` wins over ``--no-default-features``, so reusing it makes
    the leg green while testing the opposite configuration.
    """
    lines = feature_off_lines()
    assert lines, f"no {FEATURE_OFF} leg to check"

    # Every line, not just the first: the leg is an if/else pair, and the
    # fallback branch is exactly where a reuse would go unnoticed. Checking
    # only ``lines[0]`` would let a ``$(CARGO_FLAGS)`` fallback through, and
    # the sibling ``--all-features`` test cannot catch it either, because the
    # recipe text says ``$(CARGO_FLAGS)`` and the expansion happens later.
    for line in lines:
        assert "$(CARGO_FLAGS)" not in line, (
            "the leg must not reuse $(CARGO_FLAGS): its --all-features cancels "
            f"{FEATURE_OFF}, and the leg then re-enables `diagnostics` and passes "
            f"without proving anything, got {line!r}"
        )


def test_feature_off_leg_does_not_enable_all_features() -> None:
    """No route back to the feature being varied, however it is spelled."""
    for line in feature_off_lines():
        assert "--all-features" not in line, (
            f"a leg that enables every feature cannot also vary one, got {line!r}"
        )
