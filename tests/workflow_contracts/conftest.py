"""Fixtures shared by the publish-report workflow contracts.

The `build-test` job is read and parsed once per module rather than per
call, so every assertion in a module reads the same parse: a loader that
reread the file could not tell a contract failure from a file edited
mid-run. Repository access happens through
:func:`workflow_support.repository_file` and YAML parsing through
:func:`workflow_support.parse_workflow`, behind the
:func:`publish_report_support.read_build_test_job_document` boundary.

The repository configuration fixtures follow that rule for the same
reason: a module asserting on several settings reads one snapshot of the
file, and the file is opened through the same repository boundary.
"""

import tomllib
import typing as typ

import pytest
from publish_report_support import read_build_test_job_document
from workflow_support import repository_file


@pytest.fixture(scope="module")
def build_test_job() -> dict[str, typ.Any]:
    """Return the packaging job, read and parsed once for the module.

    Returns
    -------
    dict[str, typ.Any]
        The job that runs the publish dry run.

    Delegates to :func:`publish_report_support.read_build_test_job_document`,
    which documents the raised contract errors.
    """
    return read_build_test_job_document()


@pytest.fixture(scope="module")
def pyproject_configuration() -> dict[str, typ.Any]:
    """Return the parsed ``pyproject.toml``, read once for the module.

    Returns
    -------
    dict[str, typ.Any]
        The parsed configuration document.

    Delegates the read to :func:`workflow_support.repository_file`, which
    documents the raised contract errors.
    """
    return tomllib.loads(repository_file("pyproject.toml"))


@pytest.fixture(scope="module")
def makefile_text() -> str:
    """Return the ``Makefile`` text, read once for the module.

    Returns
    -------
    str
        The Makefile's contents.

    Delegates the read to :func:`workflow_support.repository_file`, which
    documents the raised contract errors.
    """
    return repository_file("Makefile")
