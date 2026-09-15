"""Tests for the generated package manifest release-contract gate."""

import typing as typ
from pathlib import Path

import pytest
from check_published_workspace_dependencies import (
    ReleaseContractError,
    parse_version,
    validate_staged_package_manifests,
)

if typ.TYPE_CHECKING:
    import collections.abc as cabc

WORKSPACE_VERSION = "0.6.1"


def write_manifest(path: Path, text: str) -> None:
    """Create *path* and write its UTF-8 TOML *text*."""
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(text, encoding="utf-8")


@pytest.fixture
def release_fixture(
    tmp_path: Path,
) -> cabc.Callable[[str], tuple[Path, Path]]:
    """Return a builder for two-crate staged release fixtures."""

    def build(patterns_requirement: str) -> tuple[Path, Path]:
        """Create one staged release fixture with a patterns requirement."""
        repository = Path(tmp_path) / "repository"
        staged_dir = repository / "target" / "published-gpui-e2e"
        write_manifest(
            repository / "Cargo.toml",
            f'[workspace.package]\nversion = "{WORKSPACE_VERSION}"\n',
        )
        write_manifest(
            repository / "lading.toml",
            '[publish]\norder = ["rstest-bdd-patterns", "rstest-bdd-macros"]\n',
        )
        write_manifest(
            staged_dir / f"rstest-bdd-patterns-{WORKSPACE_VERSION}" / "Cargo.toml",
            '[package]\nname = "rstest-bdd-patterns"\nversion = "0.6.1"\n',
        )
        write_manifest(
            staged_dir / f"rstest-bdd-macros-{WORKSPACE_VERSION}" / "Cargo.toml",
            '[package]\nname = "rstest-bdd-macros"\nversion = "0.6.1"\n'
            f'\n[dependencies]\nrstest-bdd-patterns = "{patterns_requirement}"\n',
        )
        return repository, staged_dir

    return build


def test_accepts_current_internal_dependency_lower_bound(
    release_fixture: cabc.Callable[[str], tuple[Path, Path]],
) -> None:
    """A generated manifest may depend on the current workspace release."""
    repository, staged_dir = release_fixture(WORKSPACE_VERSION)

    validate_staged_package_manifests(repository, staged_dir)


def test_rejects_stale_internal_dependency_lower_bound(
    release_fixture: cabc.Callable[[str], tuple[Path, Path]],
) -> None:
    """A generated manifest cannot resolve an older internal public API."""
    repository, staged_dir = release_fixture("0.6.0")

    with pytest.raises(
        ReleaseContractError, match=r"rstest-bdd-patterns lower bound 0\.6\.0"
    ):
        validate_staged_package_manifests(repository, staged_dir)


@pytest.mark.parametrize(
    ("requirement", "expected"),
    [
        ("0.6", (0, 6, 0)),
        ("^0", (0, 0, 0)),
        (">=0.6.1, <0.7", (0, 6, 1)),
    ],
)
def test_parses_cargo_version_lower_bounds(
    requirement: str, expected: tuple[int, int, int]
) -> None:
    """Cargo requirements retain explicit lower bounds with omitted components."""
    assert parse_version(requirement, "dependency") == expected, (
        "the release contract must retain the requirement's lower bound"
    )


@pytest.mark.parametrize("requirement", ["<0.6.1", "<=0.6.1", ">0.6.1"])
def test_rejects_requirements_without_a_lower_bound(requirement: str) -> None:
    """Upper-only requirements cannot guarantee the current release API."""
    with pytest.raises(ReleaseContractError, match="cannot determine"):
        parse_version(requirement, "dependency")
