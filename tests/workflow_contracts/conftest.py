"""Fixtures shared by the publish-report workflow contracts.

The `build-test` job is parsed once per module rather than per call, so
every assertion in a module reads the same parse: a loader that reread
the file could not tell a contract failure from a file edited mid-run.
"""

import typing as typ

import pytest
from publish_report_support import build_test_job_document


@pytest.fixture(scope="module")
def build_test_job() -> dict[str, typ.Any]:
    """Return the packaging job, parsed once for the whole module.

    Returns
    -------
    dict[str, typ.Any]
        The job that runs the publish dry run.
    """
    return build_test_job_document()
