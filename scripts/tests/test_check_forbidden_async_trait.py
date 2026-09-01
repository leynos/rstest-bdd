"""Test the async-trait ban and its two exact lockfile exemptions.

Both standalone fixtures validate published ``gpui 0.2.2`` with tracked
lockfiles. Published GPUI reaches ``async-trait`` transitively through
``ashpd -> zbus``. Every location other than the two exact lockfile paths stays
banned.
"""

import importlib
import typing as typ
from pathlib import Path

import pytest

if typ.TYPE_CHECKING:
    import collections.abc as cabc
    import types

SCRIPTS = Path(__file__).resolve().parents[1]
APPROVED_LOCKFILES = (
    "tests/fixtures/published-gpui-0-2-2/Cargo.lock",
    "tests/fixtures/published-gpui-e2e/Cargo.lock",
)

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


@pytest.mark.parametrize("approved_lockfile", APPROVED_LOCKFILES)
def test_approved_lockfile_is_ignored(
    checker: types.ModuleType, tmp_path: Path, approved_lockfile: str
) -> None:
    """Each approved published-GPUI lockfile may reference async-trait."""
    build_tree(tmp_path, {approved_lockfile: LOCK_WITH_ASYNC_TRAIT})

    assert checker.find_violations(tmp_path) == [], (
        f"approved lockfile {approved_lockfile} should be exempt from the ban"
    )


def test_root_lockfile_still_violates(
    checker: types.ModuleType, tmp_path: Path
) -> None:
    """The root workspace lockfile remains subject to the ban."""
    build_tree(tmp_path, {"Cargo.lock": LOCK_WITH_ASYNC_TRAIT})

    violations = checker.find_violations(tmp_path)

    assert violations == ["Cargo.lock: references async-trait in lockfile"], (
        "the root Cargo.lock must remain subject to the async-trait ban"
    )


def test_other_nested_lockfile_still_violates(
    checker: types.ModuleType, tmp_path: Path
) -> None:
    """A different nested lockfile is not covered by the exemption."""
    other = "tests/fixtures/other-fixture/Cargo.lock"
    build_tree(tmp_path, {other: LOCK_WITH_ASYNC_TRAIT})

    violations = checker.find_violations(tmp_path)

    assert violations == [f"{other}: references async-trait in lockfile"], (
        f"unapproved nested lockfile {other} must remain subject to the ban"
    )


@pytest.mark.parametrize(
    "nested",
    [
        "tests/fixtures/published-gpui-0-2-2/nested/Cargo.lock",
        "tests/fixtures/published-gpui-e2e/nested/Cargo.lock",
    ],
)
def test_nested_lockfile_of_approved_fixture_still_violates(
    checker: types.ModuleType, tmp_path: Path, nested: str
) -> None:
    """An approval matches one lockfile, not its fixture subtree."""
    build_tree(tmp_path, {nested: LOCK_WITH_ASYNC_TRAIT})

    violations = checker.find_violations(tmp_path)

    assert violations == [f"{nested}: references async-trait in lockfile"], (
        f"approved fixture boundary must not exempt child lockfile {nested}"
    )


@pytest.mark.parametrize(
    "manifest",
    [path.removesuffix("Cargo.lock") + "Cargo.toml" for path in APPROVED_LOCKFILES],
)
def test_approved_fixture_manifest_still_violates(
    checker: types.ModuleType, tmp_path: Path, manifest: str
) -> None:
    """Manifests inside approved fixtures are still scanned."""
    build_tree(tmp_path, {manifest: MANIFEST_WITH_ASYNC_TRAIT})

    violations = checker.find_violations(tmp_path)

    assert violations == [f"{manifest}: declares async-trait dependency"], (
        f"approved fixture manifest {manifest} must remain subject to the ban"
    )


@pytest.mark.parametrize(
    "source",
    [path.removesuffix("Cargo.lock") + "src/lib.rs" for path in APPROVED_LOCKFILES],
)
def test_approved_fixture_rust_source_still_violates(
    checker: types.ModuleType, tmp_path: Path, source: str
) -> None:
    """Rust sources inside approved fixtures are still scanned."""
    build_tree(tmp_path, {source: "use async_trait::async_trait;\n"})

    violations = checker.find_violations(tmp_path)

    assert violations == [f"{source}:1: contains forbidden async-trait usage"], (
        f"approved fixture Rust source {source} must remain subject to the ban"
    )


@pytest.mark.parametrize(
    ("contents", "expected_lines"),
    [
        ("use async_trait::async_trait;", [1]),
        ("// use async_trait::async_trait;", []),
        ("/* use async_trait::async_trait; */", []),
        ("/*\nuse async_trait::async_trait;\n*/\nuse async_trait::async_trait;", [4]),
        ("/* comment */ use async_trait::async_trait;", [1]),
    ],
    ids=[
        "code",
        "line-comment",
        "inline-block-comment",
        "multiline-block-comment",
        "after-block-comment",
    ],
)
def test_rust_comment_scanner_preserves_comment_state(
    checker: types.ModuleType,
    tmp_path: Path,
    contents: str,
    expected_lines: list[int],
) -> None:
    """Only code outside each block-comment state may contain the banned crate."""
    source = tmp_path / "src" / "lib.rs"
    source.parent.mkdir()
    source.write_text(contents, encoding="utf-8")

    assert checker.find_async_trait_in_rust(source) == expected_lines, (
        f"expected lines {expected_lines} for {contents!r}"
    )


def test_exemptions_cover_only_the_declared_paths(checker: types.ModuleType) -> None:
    """The exemption set names exactly the two published-GPUI lockfiles."""
    assert sorted(checker.EXCLUDED_LOCKFILES) == list(APPROVED_LOCKFILES), (
        "the exemption set must contain only the two approved lockfile paths"
    )
