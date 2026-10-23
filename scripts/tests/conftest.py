"""Shared pytest configuration and fixtures for the script tests.

Two things are shared here because every test beside this module needs them and
none of them owns them: the ``sys.path`` entry that lets a test import the gate
scripts directly, and the manifest writer below.

pytest imports this module automatically for the tests in its directory, so a
test obtains a fixture by naming it as a parameter. Nothing imports from here,
and no test defines these itself.

``write_manifest`` is the shared writer for a discovered fixture's manifest: a
test declares it in its signature and calls it with the temporary root and the
fixture's package name, as in::

    def test_drift(
        tmp_path: Path, write_manifest: cabc.Callable[[Path, str], Path]
    ) -> None:
        manifest = write_manifest(tmp_path, "drifted")

Routing the manifest shape through one writer is what keeps discovery's
contract in a single definition. The shape a fixture must have to be discovered
- its own ``[workspace]`` opt-out stanza and a local ``path`` dependency - is
written in exactly one place, so a test cannot drift into a shape discovery
would reject.
"""

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
