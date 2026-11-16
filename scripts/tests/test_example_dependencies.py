"""Unit tests for manifest dependency resolution."""

import typing as typ

from example_dependencies import declared_dependencies, dependency_aliases

if typ.TYPE_CHECKING:
    from pathlib import Path

MACROS_WORKSPACE_MANIFEST = """\
[workspace.dependencies]
macros = { package = "rstest-bdd-macros", version = "0.7.0" }
harness = { package = "rstest-bdd-harness", version = "0.7.0" }
"""

INHERITED_MACROS_MANIFEST = """\
[package]
name = "inherited-example"
version = "0.1.0"

[dev-dependencies]
macros.workspace = true
"""

INHERITED_HARMLESS_MANIFEST = """\
[package]
name = "inherited-harmless-example"
version = "0.1.0"

[dev-dependencies]
harness.workspace = true
"""


def write(path: Path, text: str) -> Path:
    """Write *text* to *path* and return *path*."""
    path.write_text(text, encoding="utf-8")
    return path


def test_resolves_an_inherited_alias(tmp_path: Path) -> None:
    """An inherited entry declares the crate the workspace names."""
    write(tmp_path / "Cargo.toml", MACROS_WORKSPACE_MANIFEST)
    manifest = write(tmp_path / "inherited.toml", INHERITED_MACROS_MANIFEST)

    declared = declared_dependencies(manifest, dependency_aliases(tmp_path))

    assert "rstest-bdd-macros" in declared, (
        "`macros.workspace = true` names the macro crate once the workspace says so"
    )


def test_does_not_convict_every_inherited_entry(tmp_path: Path) -> None:
    """Resolving inheritance must report only the crate actually named."""
    write(tmp_path / "Cargo.toml", MACROS_WORKSPACE_MANIFEST)
    manifest = write(tmp_path / "inherited.toml", INHERITED_HARMLESS_MANIFEST)

    declared = declared_dependencies(manifest, dependency_aliases(tmp_path))

    assert "rstest-bdd-harness" in declared, "the harness alias should resolve"
    assert "rstest-bdd-macros" not in declared, (
        "a sibling workspace alias must not be attributed to this manifest"
    )


def test_keeps_the_local_key_without_a_workspace(tmp_path: Path) -> None:
    """An unresolvable inheritance falls back to the key, inventing nothing."""
    manifest = write(tmp_path / "inherited.toml", INHERITED_MACROS_MANIFEST)

    declared = declared_dependencies(manifest, dependency_aliases(tmp_path))

    assert declared == {"macros"}, (
        "with no workspace manifest to resolve, the local key is all that is known"
    )
