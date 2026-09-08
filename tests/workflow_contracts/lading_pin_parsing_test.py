"""Behaviour of the lading pin parsers against synthetic documents.

`lading_pin_test` asserts that the repository's own four pins agree. It
cannot say what happens when one of them is malformed, because a
repository holding a malformed pin is exactly what the contract exists
to prevent. These tests supply the malformed documents directly, so the
refusals the readers promise are exercised rather than assumed.

Run via ``make test-workflow-contracts``.
"""

import pytest
from lading_pins import (
    lading_ref_in_lockfile,
    lading_ref_in_makefile,
    lading_ref_in_pyproject,
)
from publish_report_support import PublishReportShapeError
from workflow_support import MissingRepositoryFileError, repository_file

COMMIT = "0123456789abcdef0123456789abcdef01234567"
OTHER_COMMIT = "89abcdef0123456789abcdef0123456789abcdef"


def makefile_text(ref: str) -> str:
    """Return a Makefile pinning lading to one reference.

    Parameters
    ----------
    ref : str
        The reference to write into ``LADING_REF``.

    Returns
    -------
    str
        A Makefile carrying that pin among other variables.
    """
    return f"APP ?= rstest-bdd\nLADING_REF ?= {ref}\n\nall:\n\t@true\n"


def lockfile_text(*revisions: str) -> str:
    """Return a lock file resolving lading to the given revisions.

    Parameters
    ----------
    *revisions : str
        The commits to record. None yields a lock file naming no lading
        revision at all.

    Returns
    -------
    str
        A lock file fragment carrying those resolutions.
    """
    entries = "\n".join(
        f'source = {{ git = "https://github.com/leynos/lading?rev={revision}" }}'
        for revision in revisions
    )
    return f'[[package]]\nname = "lading"\n{entries}\n'


def pyproject_text(ref: str) -> str:
    """Return a project manifest pinning lading to one reference.

    Parameters
    ----------
    ref : str
        The reference to write into the project group's requirement.

    Returns
    -------
    str
        A manifest carrying that requirement.
    """
    return (
        "[dependency-groups]\npython-tools = [\n"
        f'  "lading @ git+https://github.com/leynos/lading@{ref}",\n]\n'
    )


def test_the_makefile_parser_reads_the_pin_it_declares() -> None:
    """A Makefile's ``LADING_REF`` is the commit the parser returns."""
    parsed = lading_ref_in_makefile(makefile_text(COMMIT))

    assert parsed == COMMIT, (
        f"the Makefile's LADING_REF must be read verbatim, got {parsed!r}"
    )


def test_the_lockfile_parser_reads_the_revision_it_records() -> None:
    """A lock file's single lading revision is what the parser returns."""
    parsed = lading_ref_in_lockfile(lockfile_text(COMMIT))

    assert parsed == COMMIT, (
        f"the lock file's lading revision must be read verbatim, got {parsed!r}"
    )


def test_the_pyproject_parser_reads_the_pin_it_declares() -> None:
    """The project group's commit is what the parser returns."""
    parsed = lading_ref_in_pyproject(pyproject_text(COMMIT))

    assert parsed == COMMIT, (
        f"the project group's lading pin must be read verbatim, got {parsed!r}"
    )


def test_a_makefile_without_the_pin_is_refused() -> None:
    """A Makefile that sets no ``LADING_REF`` resolves no tool at all."""
    with pytest.raises(PublishReportShapeError, match="LADING_REF"):
        lading_ref_in_makefile("APP ?= rstest-bdd\n")


def test_a_lockfile_naming_no_revision_is_refused() -> None:
    """Without a revision the lock file cannot say what `uv run` installs."""
    with pytest.raises(PublishReportShapeError, match="git revision"):
        lading_ref_in_lockfile('[[package]]\nname = "lading"\n')


def test_a_lockfile_naming_two_revisions_is_refused() -> None:
    """Two resolutions leave which one a bare `uv run` installs undecided."""
    with pytest.raises(PublishReportShapeError, match="one commit"):
        lading_ref_in_lockfile(lockfile_text(COMMIT, OTHER_COMMIT))


def test_a_pyproject_pinning_a_moving_reference_is_refused() -> None:
    """A tag lets the tool change under a pin that still reads green."""
    with pytest.raises(PublishReportShapeError, match="pin lading by commit"):
        lading_ref_in_pyproject(pyproject_text("v0.3.0"))


def test_the_repository_boundary_refuses_an_absent_file() -> None:
    """The readers' one filesystem call names the file it could not find."""
    with pytest.raises(MissingRepositoryFileError, match="no-such-pin-file"):
        repository_file("no-such-pin-file.toml")
