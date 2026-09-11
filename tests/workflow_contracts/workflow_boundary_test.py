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


def test_permission_error_is_wrapped() -> None:
    """Permission failures surface as RepositoryReadError, not OSError."""
    with (
        mock.patch(
            "pathlib.Path.read_text",
            side_effect=PermissionError(13, "Permission denied"),
        ),
        pytest.raises(RepositoryReadError) as excinfo,
    ):
        repository_file(".github", "workflows", "ci.yml")
    message = str(excinfo.value)
    assert "permission" in message, message
    assert "ci.yml" in message, message


def test_other_os_error_is_wrapped() -> None:
    """Non-permission read failures surface as RepositoryReadError."""
    with (
        mock.patch(
            "pathlib.Path.read_text",
            side_effect=OSError(5, "Input/output error"),
        ),
        pytest.raises(RepositoryReadError) as excinfo,
    ):
        repository_file("Makefile")
    message = str(excinfo.value)
    assert "unreadable" in message, message
    assert "Makefile" in message, message


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
