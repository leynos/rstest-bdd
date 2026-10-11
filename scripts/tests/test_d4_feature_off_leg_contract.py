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
design: they read the Makefile rather than invoking it, so they cost nothing.
Whether the leg actually builds the intended configuration is evidenced
separately, by running it.

A third way, which a review found after the first two were written: **the tests
can select the wrong lines to look at.** The first version located the leg by
searching the recipe for ``--no-default-features``, so a branch that *lost* the
flag was filtered out of the search results and became invisible; the surviving
branch then satisfied every assertion. The filter and the property under test
were the same string. The leg is now located structurally — by its own guard
and the ``if``/``else``/``fi`` that surround it — and the flag is asserted of
each branch, so it is never the thing that decides what gets examined.
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


def if_else_blocks(lines: list[str]) -> list[tuple[list[str], list[str]]]:
    """Split *lines* into the two branches of every ``if``/``else``/``fi``.

    The recipe decides between a nextest run and a plain ``cargo test``
    fallback, which is an ``if``/``else`` pair inside one recipe line's shell
    continuation. Reading it as a flat list of lines loses that structure, and
    losing it is what let a review slip a regression past this file.

    Parameters
    ----------
    lines : list[str]
        Recipe lines, with their leading tab already removed.

    Returns
    -------
    list[tuple[list[str], list[str]]]
        One ``(then_branch, else_branch)`` pair per block, in recipe order.
        Blocks are not nested in this recipe, so a flat scan suffices.

    A malformed block — a ``then`` not followed by both an ``else`` and a
    ``fi`` in that order — fails an assertion rather than being skipped,
    because it means the recipe is not the shape this file reasons about. That
    is a failure to report, not a case to tolerate.
    """
    blocks: list[tuple[list[str], list[str]]] = []
    index = 0
    while index < len(lines):
        if not lines[index].strip().endswith("then \\"):
            index += 1
            continue
        start = index + 1
        else_at = next(
            (i for i in range(start, len(lines)) if lines[i].strip() == "else \\"),
            None,
        )
        end = next(
            (i for i in range(start, len(lines)) if lines[i].strip() == "fi"),
            None,
        )
        assert else_at is not None, (
            "`make test` has a `then` block with no `else` to close it, so its "
            f"branches cannot be read: {lines[index]!r}"
        )
        assert end is not None, (
            "`make test` has a `then` block with no `fi` to close it, so its "
            f"branches cannot be read: {lines[index]!r}"
        )
        assert else_at < end, (
            "`make test` has a `then` block whose `else` follows its `fi`, so "
            f"its branches cannot be read: {lines[index]!r}"
        )
        blocks.append((lines[start:else_at], lines[else_at + 1 : end]))
        index = end + 1
    return blocks


def feature_off_leg() -> list[str]:
    """Return both command lines of the ``make test`` feature-off leg.

    The leg is one ``if``/``else`` pair: a nextest branch and a plain ``cargo
    test`` fallback for a machine without nextest. Both branches are returned
    whether or not they carry the flag, because returning only the lines that
    carry it is how this file previously lost the ability to catch its own
    target regression: a fallback that *drops* ``--no-default-features``
    matched no filter, vanished from the list, and left the surviving nextest
    line to satisfy every assertion on its own. The filter and the property
    under test were the same string.

    The leg is therefore located by a *different* property — the package it
    selects — so the flag is only ever asserted, never used to decide what
    gets examined.

    Returns
    -------
    list[str]
        The leg's two command lines, then-branch first.

    A recipe holding no such leg, or holding more than one, fails an assertion:
    the first is the leg-deleted regression and the second is leg-duplicated.
    Each branch being exactly one command is asserted here too, because a
    shortened leg would otherwise surface as a weaker assertion later.
    """
    blocks = [
        (then, other)
        for then, other in if_else_blocks(recipe("test"))
        if any(PACKAGE_SELECTION in line for line in (*then, *other))
    ]
    assert len(blocks) == 1, (
        f"`make test` should carry exactly one if/else leg selecting "
        f"{PACKAGE_SELECTION}, got {len(blocks)} — the leg is identified by the "
        "package it selects rather than by the feature flag, so that a branch "
        "losing the flag cannot hide itself from this search"
    )

    then, other = blocks[0]
    for name, branch in (("then", then), ("else", other)):
        assert len(branch) == 1, (
            f"the {name} branch of the {PACKAGE_SELECTION} leg must carry "
            f"exactly one command, got {branch!r}"
        )
    return [then[0], other[0]]


def test_make_test_builds_rstest_bdd_without_default_features() -> None:
    """Both branches select the right package and vary the right feature."""
    for branch, line in zip(("then", "else"), feature_off_leg(), strict=True):
        assert PACKAGE_SELECTION in line, (
            f"the {branch} branch must select the crate whose feature it "
            f"varies, got {line!r}"
        )
        assert FEATURE_OFF in line, (
            f"the {branch} branch must build with {FEATURE_OFF} — this is the "
            "assertion that a fallback losing the flag used to escape, because "
            f"the flag was the filter that found the line, got {line!r}"
        )


def test_feature_off_leg_does_not_reuse_the_shared_flag_variable() -> None:
    """The regression: ``$(CARGO_FLAGS)`` re-enables the feature being varied.

    ``CARGO_FLAGS`` is ``--workspace --all-targets --all-features``, and a later
    ``--all-features`` wins over ``--no-default-features``, so reusing it makes
    the leg green while testing the opposite configuration.
    """
    # Every branch, not just the first: the leg is an if/else pair, and the
    # fallback branch is exactly where a reuse would go unnoticed. Checking
    # only the then-branch would let a ``$(CARGO_FLAGS)`` fallback through, and
    # the sibling ``--all-features`` test cannot catch it either, because the
    # recipe text says ``$(CARGO_FLAGS)`` and the expansion happens later.
    for line in feature_off_leg():
        assert "$(CARGO_FLAGS)" not in line, (
            "the leg must not reuse $(CARGO_FLAGS): its --all-features cancels "
            f"{FEATURE_OFF}, and the leg then re-enables `diagnostics` and passes "
            f"without proving anything, got {line!r}"
        )


def test_feature_off_leg_does_not_enable_all_features() -> None:
    """No route back to the feature being varied, however it is spelled."""
    for line in feature_off_leg():
        assert "--all-features" not in line, (
            f"a leg that enables every feature cannot also vary one, got {line!r}"
        )
