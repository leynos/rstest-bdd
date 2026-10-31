"""Contract test for the development build standard (rust-build-defaults).

The Concordat ``rust-build-defaults`` rule needs the ``mold`` linker as a Linux
default in ``.cargo/config.toml``, and needs every gate recipe that assigns
``RUSTFLAGS`` in the ``Makefile`` to restate it, because an assigned
``RUSTFLAGS`` replaces every ``rustflags`` source. The Whitaker route is the
exception: its installer-managed toolchain must not inherit the flag.

Run via ``make test-workflow-contracts``.
"""

import re
import tomllib
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
LINKER_FLAG = "-Clink-arg=-fuse-ld=mold"
LINUX_TABLE = 'cfg(target_os = "linux")'


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
