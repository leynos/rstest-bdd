"""Fixtures shared by the publish-report workflow contracts.

The `build-test` job is read and parsed once per module rather than per
call, so every assertion in a module reads the same parse: a loader that
reread the file could not tell a contract failure from a file edited
mid-run. Repository access happens through
:func:`workflow_support.repository_file` and YAML parsing through
:func:`workflow_support.parse_workflow`, behind the
:func:`publish_report_support.read_build_test_job_document` boundary.
"""

import typing as typ

import pytest
from publish_report_support import read_build_test_job_document


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
