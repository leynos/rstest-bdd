"""Unit tests for the standalone fixture discovery contract.

Discovery is shared by the validation gate, the refresh path, and the mutation
lane's dependency prefetch, so these tests pin the fixture set itself rather
than the Cargo plumbing that consumes it.
"""

from pathlib import Path
from unittest import mock

import pytest
from fixture_lockfile_discovery import (
    discover_fixture_manifests,
    has_path_dependency,
    is_staged_fixture,
    is_standalone_workspace,
    is_workspace_root,
    iter_cargo_manifests,
)
from fixture_lockfile_reporting import FixtureLockfileError

REPO_ROOT = Path(__file__).resolve().parents[2]


def test_discovery_includes_feature_addition_fixture() -> None:
    """The authoritative fixture set covers the scenario-addition fixture."""
    manifests = discover_fixture_manifests(REPO_ROOT)
    names = {manifest.parent.name for manifest in manifests}
    assert "feature_addition" in names, (
        "discovery must include the feature_addition fixture; a fixture added "
        "to the set without a committed lockfile would otherwise escape the gate"
    )


def test_discovery_includes_every_committed_standalone_lockfile() -> None:
    """Every standalone lockfile on disk is covered by discovery."""
    manifests = discover_fixture_manifests(REPO_ROOT)
    lockfiles = {manifest.parent / "Cargo.lock" for manifest in manifests}
    for lockfile in (
        REPO_ROOT / "crates/cargo-bdd/tests/fixtures/minimal/Cargo.lock",
        REPO_ROOT / "crates/rstest-bdd/tests/fixtures/feature_addition/Cargo.lock",
        REPO_ROOT / "crates/rstest-bdd/tests/fixtures/rebuild_invalidation/Cargo.lock",
        REPO_ROOT / "crates/rstest-bdd/tests/ui_lints/Cargo.lock",
        REPO_ROOT / "tests/fixtures/published-gpui-0-2-2/Cargo.lock",
    ):
        assert lockfile in lockfiles, (
            f"{lockfile.relative_to(REPO_ROOT)} must stay inside the "
            "authoritative standalone fixture set"
        )


def test_discovery_excludes_the_workspace_root_and_staged_fixture() -> None:
    """The root workspace and the staged e2e fixture are out of scope."""
    manifests = discover_fixture_manifests(REPO_ROOT)
    relative = {manifest.relative_to(REPO_ROOT).as_posix() for manifest in manifests}
    assert "Cargo.toml" not in relative, (
        "the workspace root resolves through the workspace lockfile and is not "
        "a standalone fixture"
    )
    assert "tests/fixtures/published-gpui-e2e/Cargo.toml" not in relative, (
        "the staged e2e fixture resolves against target/ artefacts, not a lockfile"
    )


def test_workspace_root_is_never_a_fixture() -> None:
    """The workspace root manifest is excluded from the fixture gate."""
    assert is_workspace_root(REPO_ROOT / "Cargo.toml", REPO_ROOT), (
        "the workspace root manifest must never be treated as a fixture"
    )
    assert not is_workspace_root(
        REPO_ROOT / "crates/rstest-bdd/tests/ui_lints/Cargo.toml", REPO_ROOT
    ), "a standalone fixture manifest must stay inside the gate"


def test_staged_fixture_is_excluded_from_discovery() -> None:
    """The target/-staged e2e fixture is validated by its own targets."""
    staged = REPO_ROOT / "tests/fixtures/published-gpui-e2e/Cargo.toml"
    assert is_staged_fixture(staged, REPO_ROOT), (
        "the staged e2e fixture must be excluded from discovery"
    )
    assert not is_staged_fixture(
        REPO_ROOT / "tests/fixtures/published-gpui-0-2-2/Cargo.toml", REPO_ROOT
    ), "the published 0.2.2 fixture stays inside the gate"


def test_discovery_does_not_follow_directory_symlinks(tmp_path: Path) -> None:
    """A directory symlink cannot send the walk back up its own tree."""
    manifest = tmp_path / "fixture" / "Cargo.toml"
    manifest.parent.mkdir()
    manifest.write_text("[workspace]\n", encoding="utf-8")
    try:
        (tmp_path / "loop").symlink_to(tmp_path, target_is_directory=True)
    except (OSError, NotImplementedError) as error:
        pytest.skip(f"this platform cannot create directory symlinks: {error}")

    assert list(iter_cargo_manifests(tmp_path)) == [manifest], (
        "descending through the symlink would re-walk the same tree once per "
        "symlink the platform resolves before refusing it"
    )


def test_classification_ignores_comments_and_string_values(tmp_path: Path) -> None:
    """Comment text and string values never pass for a declaration."""
    manifest = tmp_path / "Cargo.toml"
    manifest.write_text(
        "# [workspace] with a path = ../helper dependency\n"
        "[package]\n"
        'name = "fixture"\n'
        'description = "a [workspace] with path = ../helper reference"\n',
        encoding="utf-8",
    )

    assert not is_standalone_workspace(manifest), (
        "a commented-out [workspace] stanza must not count as opting out"
    )
    assert not has_path_dependency(manifest), (
        "a path named in a comment or a string value is not a dependency"
    )


@pytest.mark.parametrize(
    "manifest_text",
    [
        '[dependencies]\nhelper = { path = "../helper" }\n',
        '[dependencies]\nhelper = {path="../helper"}\n',
        '[dependencies.helper]\npath = "../helper"\n',
        '[dev-dependencies.helper]\npath = "../helper"\n',
        '[target."cfg(unix)".dependencies]\nhelper = { path = "../helper" }\n',
        '[patch.crates-io]\nhelper = { path = "../helper" }\n',
    ],
    ids=[
        "inline-table",
        "compact-inline-table",
        "table-per-dependency",
        "dev-dependency",
        "target-qualified",
        "patch-override",
    ],
)
def test_every_path_dependency_spelling_is_recognised(
    tmp_path: Path, manifest_text: str
) -> None:
    """Each Cargo spelling of a local source counts as a path dependency."""
    manifest = tmp_path / "Cargo.toml"
    manifest.write_text(manifest_text, encoding="utf-8")

    assert has_path_dependency(manifest), (
        f"{manifest_text!r} resolves a dependency from the local filesystem"
    )


@pytest.mark.parametrize(
    "manifest_text",
    ["[workspace]\n", "[ workspace ]\n", '["workspace"]\n'],
    ids=["bare", "padded", "quoted"],
)
def test_every_workspace_section_spelling_is_recognised(
    tmp_path: Path, manifest_text: str
) -> None:
    """Each TOML spelling of the workspace table opts the fixture out."""
    manifest = tmp_path / "Cargo.toml"
    manifest.write_text(manifest_text, encoding="utf-8")

    assert is_standalone_workspace(manifest), (
        f"{manifest_text!r} declares the fixture's own workspace section"
    )


def test_classification_tolerates_invalid_toml(tmp_path: Path) -> None:
    """A manifest that is not valid TOML is classified as no fixture."""
    manifest = tmp_path / "Cargo.toml"
    manifest.write_text("[workspace\npath = ../helper\n", encoding="utf-8")

    assert not is_standalone_workspace(manifest), (
        "a manifest that is not valid TOML must not be reported as a workspace"
    )
    assert not has_path_dependency(manifest), (
        "a manifest that is not valid TOML must not be reported as a path source"
    )


def test_discovery_error_names_the_missing_contract() -> None:
    """An empty fixture set fails with a discoverable reason."""
    with (
        mock.patch("fixture_lockfile_discovery.iter_cargo_manifests", return_value=[]),
        pytest.raises(FixtureLockfileError, match="no standalone fixture"),
    ):
        discover_fixture_manifests(REPO_ROOT)
