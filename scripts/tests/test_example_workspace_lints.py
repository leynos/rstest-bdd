"""Contract tests for workspace lint inheritance in example manifests."""

from __future__ import annotations

import tomllib
from pathlib import Path

import pytest

REPO_ROOT = Path(__file__).resolve().parents[2]
EXAMPLE_MANIFESTS = (
    "examples/tokio-reminders/Cargo.toml",
    "examples/todo-cli/Cargo.toml",
    "examples/gpui-counter/Cargo.toml",
    "examples/japanese-ledger/Cargo.toml",
)


@pytest.mark.parametrize("manifest", EXAMPLE_MANIFESTS)
def test_example_manifest_inherits_workspace_lints_from_the_lints_table(
    manifest: str,
) -> None:
    """Workspace lint inheritance belongs only in each manifest's lints table."""
    with (REPO_ROOT / manifest).open("rb") as manifest_file:
        parsed = tomllib.load(manifest_file)

    assert "lints" not in parsed["package"]
    assert parsed["lints"]["workspace"] is True
