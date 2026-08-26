"""Test the async-trait ban and its single lockfile exemption.

``tests/fixtures/published-gpui-0-2-2/Cargo.lock`` is exempt because the
fixture validates the published upstream GPUI API under ``cargo check
--locked``, which requires a tracked lockfile that transitively reaches
``async-trait``. Every other location stays banned.
"""

from __future__ import annotations

import importlib
import typing as typ
from pathlib import Path

import pytest

if typ.TYPE_CHECKING:
    import collections.abc as cabc
    import types

SCRIPTS = Path(__file__).resolve().parents[1]
EXEMPT_LOCKFILE = "tests/fixtures/published-gpui-0-2-2/Cargo.lock"

LOCK_WITH_ASYNC_TRAIT = """\
version = 3

[[package]]
name = "async-trait"
version = "0.1.83"
"""

MANIFEST_WITH_ASYNC_TRAIT = """\
[package]
name = "demo"
version = "0.0.0"

[dependencies]
async-trait = "0.1"
"""


@pytest.fixture
def checker(monkeypatch: pytest.MonkeyPatch) -> types.ModuleType:
    """Import the standalone async-trait checker from the scripts directory."""
    monkeypatch.syspath_prepend(str(SCRIPTS))
    importlib.invalidate_caches()
    return importlib.import_module("check_forbidden_async_trait")


def build_tree(root: Path, files: cabc.Mapping[str, str]) -> None:
    """Write *files*, keyed by repository-relative path, beneath *root*."""
    for relative, content in files.items():
        target = root / relative
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_text(content, encoding="utf-8")


def test_exempt_lockfile_is_ignored(checker: types.ModuleType, tmp_path: Path) -> None:
    """The published-GPUI fixture lockfile may reference async-trait."""
    build_tree(tmp_path, {EXEMPT_LOCKFILE: LOCK_WITH_ASYNC_TRAIT})

    assert checker.find_violations(tmp_path) == []


def test_root_lockfile_still_violates(
    checker: types.ModuleType, tmp_path: Path
) -> None:
    """The root workspace lockfile remains subject to the ban."""
    build_tree(tmp_path, {"Cargo.lock": LOCK_WITH_ASYNC_TRAIT})

    violations = checker.find_violations(tmp_path)

    assert violations == ["Cargo.lock: references async-trait in lockfile"]


def test_other_nested_lockfile_still_violates(
    checker: types.ModuleType, tmp_path: Path
) -> None:
    """A different nested lockfile is not covered by the exemption."""
    other = "tests/fixtures/other-fixture/Cargo.lock"
    build_tree(tmp_path, {other: LOCK_WITH_ASYNC_TRAIT})

    violations = checker.find_violations(tmp_path)

    assert violations == [f"{other}: references async-trait in lockfile"]


def test_sibling_lockfile_of_exempt_fixture_still_violates(
    checker: types.ModuleType, tmp_path: Path
) -> None:
    """The exemption matches one exact path, not the fixture's whole subtree."""
    nested = "tests/fixtures/published-gpui-0-2-2/nested/Cargo.lock"
    build_tree(tmp_path, {nested: LOCK_WITH_ASYNC_TRAIT})

    violations = checker.find_violations(tmp_path)

    assert violations == [f"{nested}: references async-trait in lockfile"]


def test_exempt_fixture_manifest_still_violates(
    checker: types.ModuleType, tmp_path: Path
) -> None:
    """Manifests inside the exempt fixture are still scanned."""
    manifest = "tests/fixtures/published-gpui-0-2-2/Cargo.toml"
    build_tree(tmp_path, {manifest: MANIFEST_WITH_ASYNC_TRAIT})

    violations = checker.find_violations(tmp_path)

    assert violations == [f"{manifest}: declares async-trait dependency"]


def test_exempt_fixture_rust_source_still_violates(
    checker: types.ModuleType, tmp_path: Path
) -> None:
    """Rust sources inside the exempt fixture are still scanned."""
    source = "tests/fixtures/published-gpui-0-2-2/src/lib.rs"
    build_tree(tmp_path, {source: "use async_trait::async_trait;\n"})

    violations = checker.find_violations(tmp_path)

    assert violations == [f"{source}:1: contains forbidden async-trait usage"]


def test_exemption_covers_only_the_declared_path(checker: types.ModuleType) -> None:
    """The exemption set names exactly the one published-GPUI lockfile."""
    assert sorted(checker.EXCLUDED_LOCKFILES) == [EXEMPT_LOCKFILE]
