"""Shared fixtures for publish workspace tests."""

from __future__ import annotations

import importlib.util
import sys
import typing as typ
from pathlib import Path

import pytest

SCRIPTS_DIR = Path(__file__).resolve().parents[1]
if str(SCRIPTS_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPTS_DIR))

if typ.TYPE_CHECKING:
    from types import ModuleType


def _load_publish_workspace_module() -> ModuleType:
    spec = importlib.util.spec_from_file_location(
        "publish_workspace", SCRIPTS_DIR / "publish_workspace.py"
    )
    if spec is None or spec.loader is None:  # pragma: no cover - defensive guard
        message = "publish_workspace module could not be loaded"
        raise RuntimeError(message)
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


@pytest.fixture(scope="module")
def publish_workspace_module() -> ModuleType:
    """Provide the publish_workspace module for helper unit tests."""
    return _load_publish_workspace_module()
