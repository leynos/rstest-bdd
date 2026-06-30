"""Integration coverage for the Whitaker lint gate."""

from __future__ import annotations

import os
import shutil
import subprocess  # noqa: S404 - integration test invokes trusted local tooling.
import sys
from pathlib import Path

import pytest

REPO_ROOT = Path(__file__).resolve().parents[2]
WHITAKER_LINT = "no_unwrap_or_else_panic"
WHITAKER_TAG = "v0.2.5"
WHITAKER_TOOLCHAIN = "nightly-2025-09-18"
WHITAKER_TARGET_NAME = f"{WHITAKER_LINT}-{WHITAKER_TAG}-{WHITAKER_TOOLCHAIN}-target"
WHITAKER_TEST_TARGET = REPO_ROOT / "target" / "pytest-whitaker-fixtures"


def cargo_dylint_available() -> bool:
    """Return whether the cargo-dylint subcommand is available."""
    return shutil.which("cargo-dylint") is not None


pytestmark = pytest.mark.skipif(
    not cargo_dylint_available(),
    reason="cargo-dylint is only installed in Whitaker/tooling environments",
)


def make_executable() -> str:
    """Return the absolute Make executable used by integration tests."""
    executable = shutil.which("make")
    assert executable is not None, "make executable should be available"
    return executable


def write_lint_fixture(crate_dir: Path, lib_rs: str) -> Path:
    """Create a minimal crate for exercising the Dylint invocation."""
    (crate_dir / "src").mkdir(parents=True)
    (crate_dir / "Cargo.toml").write_text(
        "\n".join([
            "[package]",
            'name = "whitaker-lint-fixture"',
            'version = "0.0.0"',
            'edition = "2024"',
            "",
            "[lib]",
            'path = "src/lib.rs"',
            "",
        ]),
        encoding="utf-8",
    )
    (crate_dir / "src" / "lib.rs").write_text(lib_rs, encoding="utf-8")
    return crate_dir / "Cargo.toml"


def run_lint_whitaker(manifest_path: Path) -> subprocess.CompletedProcess[str]:
    """Run the Makefile target against a fixture crate."""
    env = os.environ.copy()
    env["CARGO_TARGET_DIR"] = str(WHITAKER_TEST_TARGET)
    make = make_executable()
    # The executable and arguments are controlled by this test and the repo.
    return subprocess.run(  # noqa: S603
        [
            make,
            "--no-print-directory",
            "lint-whitaker",
            f"CARGO_FLAGS=--manifest-path {manifest_path} --all-targets",
        ],
        cwd=REPO_ROOT,
        env=env,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        check=False,
        timeout=1200,
    )


def whitaker_library_name() -> str:
    """Return the expected Dylint library filename for this platform."""
    match sys.platform:
        case "darwin":
            return f"lib{WHITAKER_LINT}@{WHITAKER_TOOLCHAIN}.dylib"
        case platform if platform.startswith("linux"):
            return f"lib{WHITAKER_LINT}@{WHITAKER_TOOLCHAIN}.so"
        case "win32":
            return f"{WHITAKER_LINT}@{WHITAKER_TOOLCHAIN}.dll"
        case platform:
            return pytest.skip(f"Whitaker artefact assertion unsupported on {platform}")


def whitaker_libraries() -> list[Path]:
    """Return built Whitaker Dylint library artefacts."""
    library_root = (
        REPO_ROOT
        / "target"
        / "whitaker"
        / WHITAKER_TARGET_NAME
        / "dylint"
        / "libraries"
        / WHITAKER_TOOLCHAIN
        / "release"
    )
    return sorted(library_root.glob(whitaker_library_name()))


def test_lint_whitaker_target_accepts_clean_fixture(tmp_path: Path) -> None:
    """A clean fixture proves the Makefile target invokes cargo-dylint."""
    manifest_path = write_lint_fixture(
        tmp_path,
        "\n".join([
            "pub fn clean_value(value: Option<u32>) -> u32 {",
            "    let Some(number) = value else {",
            '        panic!("missing value");',
            "    };",
            "    number",
            "}",
            "",
        ]),
    )

    result = run_lint_whitaker(manifest_path)

    assert result.returncode == 0, result.stdout
    assert whitaker_libraries(), "Whitaker lint library should be built"


def test_lint_whitaker_target_rejects_panicking_unwrap_or_else(
    tmp_path: Path,
) -> None:
    """A bad fixture proves the gate detects the banned panic shape."""
    manifest_path = write_lint_fixture(
        tmp_path,
        "\n".join([
            "pub fn rejected_value(value: Option<u32>) -> u32 {",
            '    value.unwrap_or_else(|| panic!("missing value"))',
            "}",
            "",
        ]),
    )

    result = run_lint_whitaker(manifest_path)

    assert result.returncode != 0, result.stdout
    assert WHITAKER_LINT in result.stdout, result.stdout
    assert "unwrap_or_else" in result.stdout, result.stdout
