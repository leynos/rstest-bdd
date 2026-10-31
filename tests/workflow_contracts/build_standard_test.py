"""Contract test for the development build standard (rust-build-defaults).

The Concordat ``rust-build-defaults`` rule needs the ``mold`` linker as a Linux
default in ``.cargo/config.toml``, and needs every gate recipe that assigns
``RUSTFLAGS`` in the ``Makefile`` to restate it, because an assigned
``RUSTFLAGS`` replaces every ``rustflags`` source. The Whitaker route is the
exception: its installer-managed toolchain must not inherit the flag.

Run via ``make test-workflow-contracts``.
"""

import os
import re
import subprocess  # ruff: ignore[suspicious-subprocess-import] - dry-runs this repository's own Makefile.
import tomllib
from pathlib import Path

import pytest

ROOT = Path(__file__).resolve().parents[2]
LINKER_FLAG = "-Clink-arg=-fuse-ld=mold"
LINUX_TABLE = 'cfg(target_os = "linux")'

#: Every Cargo invocation each gate target prints under ``make -n``, in order:
#: the leading subcommand words. ``test`` builds the binaries, then runs the
#: workspace suite (nextest when installed, otherwise ``cargo test``), the
#: doctests, and the no-default-features leg; ``typecheck`` runs ``cargo check``.
EXPECTED_INVOCATIONS = {
    "test": [
        "build",
        "nextest run",
        "test",
        "test --doc",
        "nextest run",
        "test",
    ],
    "typecheck": ["check"],
}
ASSIGNED_CARGO = re.compile(
    r'RUSTFLAGS="(?P<flags>[^"]*)"\s+\S*cargo\s+(?P<sub>nextest run|test --doc|\w+)'
)


def test_linker_is_a_linux_only_default() -> None:
    """The linker flag lives in the Linux table and in no platform-wide source."""
    config = tomllib.loads((ROOT / ".cargo" / "config.toml").read_text("utf-8"))
    linux = config.get("target", {}).get(LINUX_TABLE, {})
    assert LINKER_FLAG in linux.get("rustflags", []), (
        "the Linux table must select the mold linker"
    )
    build = config.get("build", {}).get("rustflags", [])
    assert LINKER_FLAG not in build, (
        "the mold linker ships for Linux only, so [build] must not name it"
    )


def _recipe(makefile: str, target: str) -> str:
    """Return the text of one Makefile target's recipe."""
    pattern = rf"^{re.escape(target)}:[^\n]*\n((?:\t[^\n]*\n|\n(?=\t))*)"
    match = re.search(pattern, makefile, re.MULTILINE)
    assert match, f"the Makefile must define {target}"
    return match.group(1)


def test_gate_recipes_restate_the_linker_flag() -> None:
    """The test and typecheck recipes add the standard flags to RUSTFLAGS."""
    makefile = (ROOT / "Makefile").read_text("utf-8")
    assert (
        f"STANDARD_RUSTFLAGS := $(if $(filter Linux,$(BUILD_HOST_OS)),{LINKER_FLAG})"
        in makefile
    ), "STANDARD_RUSTFLAGS must carry the mold linker on Linux only"
    restated = 'RUSTFLAGS="$(RUST_FLAGS) $(STANDARD_RUSTFLAGS)"'
    for target in ("test", "typecheck"):
        recipe = _recipe(makefile, target)
        assert restated in recipe, f"{target} must restate the linker flag"
        assert 'RUSTFLAGS="$(RUST_FLAGS)"' not in recipe, f"{target} must not drop it"


def test_whitaker_route_takes_no_development_flags() -> None:
    """The Whitaker recipe assigns plain warnings only."""
    recipe = _recipe((ROOT / "Makefile").read_text("utf-8"), "lint-whitaker")
    assert "STANDARD_RUSTFLAGS" not in recipe, (
        "the Whitaker route must not inherit the mold linker"
    )


def _dry_run(target: str, host_os: str) -> str:
    """Return what ``make -n`` prints for a gate target on a controlled host."""
    return subprocess.run(  # ruff: ignore[subprocess-without-shell-equals-true] - a fixed Make invocation.
        ["make", "--dry-run", target, f"BUILD_HOST_OS={host_os}"],  # ruff: ignore[start-process-with-partial-path] - make is resolved from PATH.
        cwd=ROOT,
        env=os.environ.copy(),
        capture_output=True,
        check=True,
        text=True,
    ).stdout


@pytest.mark.parametrize("target", sorted(EXPECTED_INVOCATIONS))
@pytest.mark.parametrize(
    ("host_os", "expects_linker"),
    [("Linux", True), ("Darwin", False), ("Windows_NT", False)],
)
def test_every_cargo_invocation_in_a_gate_target_restates_the_flags(
    target: str, host_os: str, *, expects_linker: bool
) -> None:
    """Each Cargo call assigns the warning policy, plus the linker on Linux only.

    The host is controlled through ``BUILD_HOST_OS``, so both branches of
    ``STANDARD_RUSTFLAGS`` run on any machine. The enumeration is exact, so a
    new Cargo call that forgets the assignment, or a dropped one, is noticed.
    """
    found = ASSIGNED_CARGO.findall(_dry_run(target, host_os))
    assert [sub for _, sub in found] == EXPECTED_INVOCATIONS[target], (
        f"make {target} on {host_os} must assign RUSTFLAGS on every Cargo call"
    )
    expected = ["-D", "warnings"] + ([LINKER_FLAG] if expects_linker else [])
    for flags, sub in found:
        assert flags.split() == expected, (
            f"make {target} on {host_os}: `cargo {sub}` must assign exactly {expected}"
        )
