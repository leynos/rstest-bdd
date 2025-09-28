"""General workflow integration coverage for ``run_publish_check``."""

from __future__ import annotations

import typing as typ

import pytest

from .conftest import WorkflowTestConfig, _setup_basic_workflow_mocks

if typ.TYPE_CHECKING:
    from pathlib import Path
    from types import ModuleType


def test_run_publish_check_preserves_workspace(
    monkeypatch: pytest.MonkeyPatch,
    tmp_path: Path,
    capsys: pytest.CaptureFixture[str],
    run_publish_check_module: ModuleType,
) -> None:
    """Ensure the workflow keeps the workspace when requested."""
    workspace_dir = _setup_basic_workflow_mocks(
        monkeypatch,
        tmp_path,
        run_publish_check_module,
        config=WorkflowTestConfig(workspace_name="persist"),
    )

    run_publish_check_module.run_publish_check(keep_tmp=True, timeout_secs=5)

    captured = capsys.readouterr()
    assert "preserving workspace" in captured.out
    assert workspace_dir.exists()


def test_run_publish_check_errors_when_crate_order_empty(
    monkeypatch: pytest.MonkeyPatch,
    tmp_path: Path,
    run_publish_check_module: ModuleType,
) -> None:
    """Verify the workflow aborts if ``CRATE_ORDER`` is empty."""
    _setup_basic_workflow_mocks(
        monkeypatch,
        tmp_path,
        run_publish_check_module,
        config=WorkflowTestConfig(
            workspace_name="missing-order",
            crate_order=(),
        ),
    )

    with pytest.raises(SystemExit, match="CRATE_ORDER must not be empty"):
        run_publish_check_module.run_publish_check(keep_tmp=False, timeout_secs=5)


def test_run_publish_check_rejects_non_positive_timeout(
    run_publish_check_module: ModuleType,
) -> None:
    """Reject configurations that specify a timeout below one second."""
    with pytest.raises(SystemExit, match="positive integer"):
        run_publish_check_module.run_publish_check(keep_tmp=False, timeout_secs=0)
