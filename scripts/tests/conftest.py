"""Shared script-test configuration and fixtures."""

import sys
import typing as typ
from pathlib import Path

import pytest

if typ.TYPE_CHECKING:
    import collections.abc as cabc

SCRIPTS_DIR = Path(__file__).resolve().parents[1]
if str(SCRIPTS_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPTS_DIR))


def write_fixture_manifest(root: Path, package: str) -> Path:
    """Write a standalone fixture manifest under *root*, returning its path.

    Discovery requires the shape a real fixture has: its own ``[workspace]``
    opt-out stanza and a local ``path`` dependency. Writing that shape from one
    place keeps the manifest contract in a single definition, so a test that
    seeds a temporary fixture exercises the real check path and cannot drift
    into a shape discovery would reject.

    Parameters
    ----------
    root : Path
        The temporary repository root to write the fixture beneath.
    package : str
        The fixture directory name, which also names its crate.

    Returns
    -------
    Path
        The manifest written for the fixture.
    """
    fixture = root / "fixtures" / package
    fixture.mkdir(parents=True)
    manifest = fixture / "Cargo.toml"
    manifest.write_text(
        f'[package]\nname = "{package}-fixture"\nversion = "0.1.0"\n'
        f'[dependencies]\nlocal = {{ path = "../../local" }}\n[workspace]\n',
        encoding="utf-8",
    )
    return manifest


@pytest.fixture
def write_manifest() -> cabc.Callable[[Path, str], Path]:
    """Return the shared writer for a discovered fixture's manifest.

    The writer is injected rather than imported so a test declares the
    dependency it has on the fixture's manifest shape. Only the lockfile
    contents and the assertions stay local to each test, because those are what
    differ between the scenarios.

    Returns
    -------
    cabc.Callable[[Path, str], Path]
        A writer taking the temporary root and package name, and answering
        with the manifest it wrote.
    """
    return write_fixture_manifest
