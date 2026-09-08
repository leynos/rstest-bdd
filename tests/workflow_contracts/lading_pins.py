"""Reads the four places lading's pin is written.

Separated from `publish_report_support` so the pin readers and the
publish-report vocabulary stay legible apart, and so neither module
outgrows the 400-line limit the Python lint gate enforces.

Each pin is read in two halves: a pure function that parses supplied
text, and a one-line reader that fetches that text through
:func:`workflow_support.repository_file` and delegates. The parsers are
therefore testable against synthetic documents, including the malformed
ones no repository file will ever hold, and the only filesystem access
in this module is the single call each reader makes. A reader
propagates :class:`workflow_support.MissingRepositoryFileError` when the
file it names is absent.

Run via ``make test-workflow-contracts``.
"""

import re

from publish_report_support import PublishReportShapeError, _require
from workflow_support import repository_file

LOCKFILE_REVISION = re.compile(r"github\.com/leynos/lading\?rev=([0-9a-f]{40})")
MAKEFILE_PIN = re.compile(r"^LADING_REF \?= (?P<ref>\S+)$", re.MULTILINE)
PYPROJECT_PIN = re.compile(
    r"lading @ git\+https://github\.com/leynos/lading@(?P<ref>[0-9a-f]{40})"
)


def lading_ref_in_makefile(makefile: str) -> str:
    """Return the lading pin a Makefile's text declares.

    Parameters
    ----------
    makefile : str
        The text of a Makefile.

    Returns
    -------
    str
        The commit the Makefile resolves lading from.

    Raises
    ------
    PublishReportShapeError
        If the text defines no ``LADING_REF``.
    """
    match = MAKEFILE_PIN.search(makefile)
    if match is None:
        message = "the Makefile must define LADING_REF"
        raise PublishReportShapeError(message)
    return match["ref"]


def lading_ref_in_lockfile(lockfile: str) -> str:
    """Return the commit a lock file's text resolves lading to.

    The fourth place the pin lives, and the one that decides what a bare
    ``uv run`` actually installs: the project group states a commit and
    the lock file records the one resolution chose. They can disagree
    only through an incomplete bump, and the failure is silent, because
    the lock file wins.

    Text recording no lading revision, or resolving lading to more than
    one, raises :class:`PublishReportShapeError` through :func:`_require`:
    either way the lock file cannot say what a bare ``uv run`` installs.
    The lint refuses a ``Raises`` section for an exception a function
    does not raise directly, so it is stated here.

    Parameters
    ----------
    lockfile : str
        The text of a ``uv`` lock file.

    Returns
    -------
    str
        The commit recorded in the lock file.
    """
    matches = set(LOCKFILE_REVISION.findall(lockfile))
    _require(matches, "uv.lock must record a lading git revision")
    _require(
        len(matches) == 1,
        f"uv.lock must resolve lading to one commit, found {sorted(matches)}",
    )
    return matches.pop()


def lading_ref_in_pyproject(pyproject: str) -> str:
    """Return the lading pin a project manifest's text declares.

    Parameters
    ----------
    pyproject : str
        The text of a ``pyproject.toml``.

    Returns
    -------
    str
        The commit `uv` resolves lading from for a bare `uv run`.

    Raises
    ------
    PublishReportShapeError
        If the text does not pin lading by commit.
    """
    match = PYPROJECT_PIN.search(pyproject)
    if match is None:
        message = "pyproject.toml must pin lading by commit"
        raise PublishReportShapeError(message)
    return match["ref"]


def makefile_lading_ref() -> str:
    """Return the repository Makefile's lading pin.

    Reads ``Makefile`` through the repository boundary and parses it with
    :func:`lading_ref_in_makefile`, so a missing file raises
    :class:`workflow_support.MissingRepositoryFileError` and a file
    without the pin raises :class:`PublishReportShapeError`.

    Returns
    -------
    str
        The commit the Makefile resolves lading from.
    """
    return lading_ref_in_makefile(repository_file("Makefile"))


def lockfile_lading_ref() -> str:
    """Return the commit the repository's `uv.lock` resolves lading to.

    Reads ``uv.lock`` through the repository boundary and parses it with
    :func:`lading_ref_in_lockfile`, so a missing file raises
    :class:`workflow_support.MissingRepositoryFileError` and a lock file
    naming no revision, or more than one, raises
    :class:`PublishReportShapeError`.

    Returns
    -------
    str
        The commit recorded in the lock file.
    """
    return lading_ref_in_lockfile(repository_file("uv.lock"))


def pyproject_lading_ref() -> str:
    """Return the repository project group's lading pin.

    Reads ``pyproject.toml`` through the repository boundary and parses
    it with :func:`lading_ref_in_pyproject`, so a missing file raises
    :class:`workflow_support.MissingRepositoryFileError` and a manifest
    without a commit pin raises :class:`PublishReportShapeError`.

    Returns
    -------
    str
        The commit `uv` resolves lading from for a bare `uv run`.
    """
    return lading_ref_in_pyproject(repository_file("pyproject.toml"))
