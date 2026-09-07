"""Test the workspace ``unsafe_code``-suppression policy guard."""

import importlib
import typing as typ
from pathlib import Path

import pytest

if typ.TYPE_CHECKING:
    import types

SCRIPTS = Path(__file__).resolve().parents[1]


@pytest.fixture
def checker(monkeypatch: pytest.MonkeyPatch) -> types.ModuleType:
    """Import the standalone unsafe-code policy checker from ``scripts``."""
    monkeypatch.syspath_prepend(str(SCRIPTS))
    importlib.invalidate_caches()
    return importlib.import_module("check_unsafe_code_allows")


def write_source(root: Path, relative: str, contents: str) -> None:
    """Create one Rust source file beneath the temporary workspace tree."""
    source = root / relative
    source.parent.mkdir(parents=True, exist_ok=True)
    source.write_text(contents, encoding="utf-8")


def test_allows_workspace_source_without_unsafe_code_suppression(
    checker: types.ModuleType, tmp_path: Path
) -> None:
    """Ordinary workspace Rust source passes the policy guard."""
    write_source(tmp_path, "crates/demo/src/lib.rs", "pub fn answer() -> u8 { 42 }\n")

    assert checker.find_violations(tmp_path) == [], "safe workspace source should pass"
    assert checker.main(tmp_path) == 0, "safe workspace source should exit successfully"


@pytest.mark.parametrize(
    "attribute",
    [
        "#[allow(unsafe_code)]",
        "#![allow(unsafe_code)]",
    ],
    ids=["item-attribute", "crate-attribute"],
)
def test_rejects_workspace_unsafe_code_suppression(
    checker: types.ModuleType,
    tmp_path: Path,
    capsys: pytest.CaptureFixture[str],
    attribute: str,
) -> None:
    """Both item and crate attributes fail with the offending source path."""
    write_source(
        tmp_path, "crates/demo/src/lib.rs", f"{attribute}\npub fn answer() {{}}\n"
    )

    assert checker.main(tmp_path) == 1, "unsafe-code suppression should fail"

    captured = capsys.readouterr()
    assert f"crates/demo/src/lib.rs:1: {attribute}" in captured.err, (
        "the rejection should identify the offending source path"
    )
    assert (
        "unsafe_code exceptions require an explicit policy decision" in captured.err
    ), "the rejection should explain the required policy decision"


def test_excludes_generated_and_dependency_directories(
    checker: types.ModuleType, tmp_path: Path
) -> None:
    """Generated artefacts and vendored dependencies remain outside the guard."""
    write_source(tmp_path, "target/generated.rs", "#[allow(unsafe_code)]\n")
    write_source(tmp_path, "vendor/dependency/src/lib.rs", "#![allow(unsafe_code)]\n")

    assert checker.find_violations(tmp_path) == [], (
        "generated and dependency directories should not be scanned"
    )
