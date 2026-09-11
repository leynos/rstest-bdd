"""Contract for the lading pin, in every place it lives.

lading runs the publish dry run, and it is pinned four times: `ci.yml`
sets `LADING_REF` for the job, the Makefile sets it again so a local
`make publish-check` resolves the same tool, `pyproject.toml` pins it in
the `python-tools` group for a bare `uv run lading`, and `uv.lock`
records the commit resolution actually chose.

Four pins for one tool drift, and the failure is quiet: CI validates
publish readiness with one version while a developer validates it with
another, and each believes the other agrees. The project group is the
easiest to forget, because the Makefile's `--with` overlay masks it; the
lock file is the one that decides what a bare `uv run` installs.

The version itself is not asserted. A bump should need one paired
change, not four. What is asserted is that they agree, and that the pin
is a commit rather than a moving reference.

Run via ``make test-workflow-contracts``.
"""

import typing as typ

from lading_pins import (
    lockfile_lading_ref,
    makefile_lading_ref,
    pyproject_lading_ref,
)
from publish_report_support import FULL_SHA as _FULL_SHA
from publish_report_support import build_test_env


def test_every_lading_pin_agrees(build_test_job: dict[str, typ.Any]) -> None:
    """All four places must resolve the same lading.

    Drift here is quiet rather than loud: every side keeps working, and
    each validates publish readiness against a different tool while
    believing the others agree. The project group is the easiest to
    forget, because the Makefile's `--with` overlay hides it, and the
    lock file is the one that decides what a bare `uv run` installs, so
    a bump that moved the group and not the lock would leave the group's
    commit stated and another one used.
    """
    pins = {
        "ci.yml": build_test_env(build_test_job).get("LADING_REF"),
        "Makefile": makefile_lading_ref(),
        "pyproject.toml": pyproject_lading_ref(),
        "uv.lock": lockfile_lading_ref(),
    }

    assert len(set(pins.values())) == 1, (
        f"every lading pin must name the same commit; a bump must move all "
        f"of them: {pins}"
    )


def test_the_lading_pin_is_a_commit() -> None:
    """A tag or branch would let the tool change under a green pin."""
    makefile_ref = makefile_lading_ref()

    assert _FULL_SHA.match(makefile_ref), (
        f"LADING_REF must be a full 40-character commit SHA, not a tag or "
        f"branch: {makefile_ref!r}"
    )
