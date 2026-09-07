"""Reads the four places lading's pin is written.

Separated from `publish_report_support` so the pin readers and the
publish-report vocabulary stay legible apart, and so neither module
outgrows the 400-line limit the Python lint gate enforces.

Run via ``make test-workflow-contracts``.
"""

import re

from publish_report_support import PublishReportShapeError, _require
from workflow_support import repository_file


def makefile_lading_ref() -> str:
    """Return the Makefile's lading pin.

    Returns
    -------
    str
        The commit the Makefile resolves lading from.

    Raises
    ------
    PublishReportShapeError
        If the Makefile defines no ``LADING_REF``.
    """
    makefile = repository_file("Makefile")
    match = re.search(r"^LADING_REF \?= (?P<ref>\S+)$", makefile, re.MULTILINE)
    if match is None:
        message = "the Makefile must define LADING_REF"
        raise PublishReportShapeError(message)
    return match["ref"]


def lockfile_lading_ref() -> str:
    """Return the commit `uv.lock` resolves Lading to.

    The fourth place the pin lives, and the one that decides what a bare
    ``uv run`` actually installs: the project group states a commit and
    the lock file records the one resolution chose. They can disagree
    only through an incomplete bump, and the failure is silent, because
    the lock file wins.

    A lock file recording no Lading revision, or resolving Lading to
    more than one, raises :class:`PublishReportShapeError` through
    :func:`_require`: either way the lock file cannot say what a bare
    ``uv run`` installs. The lint refuses a ``Raises`` section for an
    exception a function does not raise directly, so it is stated here.

    Returns
    -------
    str
        The commit recorded in the lock file.
    """
    lockfile = repository_file("uv.lock")
    matches = set(
        re.findall(
            r"github\.com/leynos/lading\?rev=([0-9a-f]{40})",
            lockfile,
        )
    )
    _require(matches, "uv.lock must record a lading git revision")
    _require(
        len(matches) == 1,
        f"uv.lock must resolve lading to one commit, found {sorted(matches)}",
    )
    return matches.pop()


def pyproject_lading_ref() -> str:
    """Return the project group's lading pin.

    Returns
    -------
    str
        The commit `uv` resolves lading from for a bare `uv run`.

    Raises
    ------
    PublishReportShapeError
        If `pyproject.toml` does not pin lading by commit.
    """
    pyproject = repository_file("pyproject.toml")
    match = re.search(
        r"lading @ git\+https://github\.com/leynos/lading@(?P<ref>[0-9a-f]{40})",
        pyproject,
    )
    if match is None:
        message = "pyproject.toml must pin lading by commit"
        raise PublishReportShapeError(message)
    return match["ref"]
