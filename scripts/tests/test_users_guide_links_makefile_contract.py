"""Contract tests for the Makefile wiring around the users-guide links.

The acceptance criterion for issue #537 is that one command regenerates the
guide's reference block and that ``make lint`` fails while the committed block
disagrees with it, so the two Makefile steps are asserted here rather than
left to prose.
"""

from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[2]
MAKEFILE = REPO_ROOT / "Makefile"
CHECKER = "scripts/check_users_guide_links.py"


def recipe(target: str) -> list[str]:
    """Return the recipe lines of *target* in the repository Makefile.

    Returns
    -------
    list[str]
        The tab-indented recipe lines, without their leading tab. An absent or
        duplicated target fails the assertion above rather than returning.
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


def test_lint_validates_the_guide_without_rewriting_it() -> None:
    """``make lint`` checks the committed reference block as written."""
    lines = [line for line in recipe("lint") if CHECKER in line]

    assert len(lines) == 1, f"lint should check the guide once, got {lines!r}"
    assert "--fix" not in lines[0], (
        f"lint must not rewrite the guide, got {lines[0]!r}; regeneration is the"
        " update-users-guide-links target's job"
    )


def test_update_target_rewrites_the_guide_from_the_canonical_base_url() -> None:
    """``make update-users-guide-links`` is the write side of that check."""
    lines = recipe("update-users-guide-links")

    assert any(CHECKER in line and "--fix" in line for line in lines), (
        f"the update target should regenerate the guide, got {lines!r}"
    )


def test_update_target_is_phony() -> None:
    """The target names a command, not a file, so it must be declared phony."""
    phony = [
        line
        for line in MAKEFILE.read_text(encoding="utf-8").splitlines()
        if line.startswith(".PHONY:")
    ]

    assert any("update-users-guide-links" in line for line in phony), (
        f"update-users-guide-links should be .PHONY, got {phony!r}"
    )


def test_both_steps_use_the_project_python() -> None:
    """Both steps run through the pinned interpreter, not a bare python3."""
    for target in ("lint", "update-users-guide-links"):
        lines = [line for line in recipe(target) if CHECKER in line]
        assert lines, f"{target} should run the checker, got {lines!r}"
        assert all("$(PROJECT_PYTHON)" in line for line in lines), (
            f"{target} should run the checker through $(PROJECT_PYTHON), got {lines!r}"
        )
