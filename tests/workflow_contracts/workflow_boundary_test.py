"""Focused contracts for the workflow-support parse and repository boundary."""

from unittest import mock

import pytest
from workflow_support import (
    MissingRepositoryFileError,
    NotAMappingError,
    RepositoryReadError,
    job_from_document,
    parse_workflow,
    repository_file,
)

MINIMAL_WORKFLOW = """\
name: CI
jobs:
  build:
    steps:
      - run: make test
"""


def test_missing_file_raises_missing_repository_file_error() -> None:
    """Absent repository paths surface as MissingRepositoryFileError."""
    with pytest.raises(MissingRepositoryFileError) as excinfo:
        repository_file(".github", "workflows", "definitely-absent.yml")
    message = str(excinfo.value)
    assert "must exist" in message, message
    assert "definitely-absent.yml" in message, message


@pytest.mark.parametrize(
    ("read_failure", "parts", "category_fragment", "path_fragment"),
    [
        pytest.param(
            PermissionError(13, "Permission denied"),
            (".github", "workflows", "ci.yml"),
            "permission",
            "ci.yml",
            id="permission-error",
        ),
        pytest.param(
            OSError(5, "Input/output error"),
            ("Makefile",),
            "unreadable",
            "Makefile",
            id="other-os-error",
        ),
    ],
)
def test_read_failure_is_wrapped(
    read_failure: Exception,
    parts: tuple[str, ...],
    category_fragment: str,
    path_fragment: str,
) -> None:
    """Filesystem read failures surface as RepositoryReadError, not OSError."""
    with (
        mock.patch("pathlib.Path.read_text", side_effect=read_failure),
        pytest.raises(RepositoryReadError) as excinfo,
    ):
        repository_file(*parts)
    message = str(excinfo.value)
    assert category_fragment in message, message
    assert path_fragment in message, message


def test_unicode_decode_error_is_wrapped() -> None:
    """Non-UTF-8 bytes surface as RepositoryReadError, not UnicodeDecodeError."""
    decode_error = UnicodeDecodeError("utf-8", b"\x00\xff", 0, 1, "invalid start byte")
    with (
        mock.patch(
            "pathlib.Path.read_text",
            side_effect=decode_error,
        ),
        pytest.raises(RepositoryReadError) as excinfo,
    ):
        repository_file(".github", "workflows", "ci.yml")
    message = str(excinfo.value)
    assert "utf-8" in message.lower(), message
    assert "ci.yml" in message, message


def test_parse_workflow_rejects_non_mapping() -> None:
    """A YAML scalar or sequence document violates the mapping contract."""
    with pytest.raises(NotAMappingError):
        parse_workflow("- just\n- a\n- list\n")


def test_parse_workflow_accepts_mapping() -> None:
    """A mapping document parses to a dict."""
    document = parse_workflow(MINIMAL_WORKFLOW)
    assert document["name"] == "CI", document


def test_job_from_document_extracts_named_job() -> None:
    """job_from_document extracts the job under the given name."""
    document = parse_workflow(MINIMAL_WORKFLOW)
    job = job_from_document(document, "build")
    assert job["steps"] == [{"run": "make test"}], job["steps"]
