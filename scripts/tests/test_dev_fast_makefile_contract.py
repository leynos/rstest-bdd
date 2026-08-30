"""Integration coverage for the opt-in dev-fast Makefile targets."""

from __future__ import annotations

import dataclasses as dc
import os
import re
import shutil
import stat
import subprocess  # noqa: S404 - integration test invokes the trusted local Makefile.
import sys
import tomllib
import typing as typ
from pathlib import Path

if typ.TYPE_CHECKING:
    import pytest

REPO_ROOT = Path(__file__).resolve().parents[2]
MAKEFILE = REPO_ROOT / "Makefile"
CARGO_CONFIG_FILE = REPO_ROOT / ".cargo" / "config.toml"


def make_variable(name: str) -> str:
    """Return the default value assigned to a Makefile variable."""
    match = re.search(
        rf"^{re.escape(name)}\s*\?=\s*(?P<value>.+?)\s*$",
        MAKEFILE.read_text(encoding="utf-8"),
        flags=re.MULTILINE,
    )
    assert match is not None, f"{name} should have a default in the Makefile"
    return match.group("value")


DEV_FAST_TOOLCHAIN = make_variable("DEV_FAST_TOOLCHAIN")
DEV_FAST_CONFIG = make_variable("DEV_FAST_CONFIG")
DEV_FAST_CONFIG_FILE = REPO_ROOT / DEV_FAST_CONFIG


@dc.dataclass(frozen=True)
class DevFastInvocation:
    """The fake Cargo call and its Makefile execution result."""

    fake_cargo: Path
    invocation_log: Path
    result: subprocess.CompletedProcess[str]
    real_cargo: str | None


def make_executable() -> str:
    """Return the absolute Make executable used by integration tests."""
    executable = shutil.which("make")
    assert executable is not None, "make executable should be available"
    return executable


def write_fake_cargo(tmp_path: Path) -> tuple[Path, Path]:
    """Create a Cargo stand-in that records each received argument vector."""
    invocation_log = tmp_path / "cargo-invocations.log"
    fake_cargo = tmp_path / "cargo"
    fake_cargo.write_text(
        "\n".join([
            f"#!{sys.executable}",
            "import os",
            "import sys",
            "from pathlib import Path",
            "",
            (
                'with Path(os.environ["FAKE_CARGO_INVOCATIONS"]).open('
                '"a", encoding="utf-8") as invocation_log:'
            ),
            '    invocation_log.write("\\0".join(sys.argv) + "\\n")',
            "",
        ]),
        encoding="utf-8",
    )
    fake_cargo.chmod(fake_cargo.stat().st_mode | stat.S_IXUSR)
    return fake_cargo, invocation_log


def invoke_dev_fast_target(
    target: str, tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> DevFastInvocation:
    """Run one dev-fast target with Cargo replaced by a recording executable."""
    fake_cargo, invocation_log = write_fake_cargo(tmp_path)
    original_path = os.environ["PATH"]
    real_cargo = shutil.which("cargo", path=original_path)
    monkeypatch.setenv("PATH", f"{tmp_path}{os.pathsep}{original_path}")
    monkeypatch.setenv("CARGO", str(fake_cargo))
    monkeypatch.setenv("FAKE_CARGO_INVOCATIONS", str(invocation_log))
    result = subprocess.run(  # noqa: S603 - target and command are controlled by this test.
        [make_executable(), "--no-print-directory", target],
        cwd=REPO_ROOT,
        env=os.environ.copy(),
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        check=False,
        timeout=30,
    )
    return DevFastInvocation(
        fake_cargo=fake_cargo,
        invocation_log=invocation_log,
        result=result,
        real_cargo=real_cargo,
    )


def assert_cargo_invocation(
    *,
    invocation: DevFastInvocation,
    expected_subcommand: str,
) -> None:
    """Assert that a dev-fast target delegates only to the recording Cargo."""
    assert invocation.result.returncode == 0, invocation.result.stdout
    records = [
        record
        for record in invocation.invocation_log.read_text(encoding="utf-8").splitlines()
        if record
    ]
    assert len(records) == 1
    executable, *arguments = records[0].split("\0")
    assert Path(executable).resolve() == invocation.fake_cargo.resolve()
    if invocation.real_cargo is not None:
        assert Path(executable).resolve() != Path(invocation.real_cargo).resolve()
    assert arguments == [
        f"+{DEV_FAST_TOOLCHAIN}",
        "--config",
        DEV_FAST_CONFIG,
        expected_subcommand,
    ]


def test_dev_build_uses_the_dev_fast_cargo_contract(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    """The build target selects the pinned nightly and explicit fragment."""
    invocation = invoke_dev_fast_target("dev-build", tmp_path, monkeypatch)

    assert_cargo_invocation(
        invocation=invocation,
        expected_subcommand="build",
    )


def test_dev_test_uses_the_dev_fast_cargo_contract(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    """The test target selects the pinned nightly and explicit fragment."""
    invocation = invoke_dev_fast_target("dev-test", tmp_path, monkeypatch)

    assert_cargo_invocation(
        invocation=invocation,
        expected_subcommand="test",
    )


def test_dev_fast_configuration_enables_the_expected_backends() -> None:
    """The opt-in fragment selects Cranelift and mold only where supported."""
    with DEV_FAST_CONFIG_FILE.open("rb") as config_file:
        config = tomllib.load(config_file)

    assert config["unstable"]["codegen-backend"] is True
    assert config["profile"]["dev"]["codegen-backend"] == "cranelift"
    assert (
        "-Clink-arg=-fuse-ld=mold"
        in config["target"]['cfg(target_os = "linux")']["rustflags"]
    )


def test_cargo_auto_configuration_excludes_the_dev_fast_fragment() -> None:
    """Auto-discovered Cargo configuration cannot enable the opt-in fragment."""
    if not CARGO_CONFIG_FILE.exists():
        return

    cargo_config = CARGO_CONFIG_FILE.read_text(encoding="utf-8")
    assert DEV_FAST_CONFIG not in cargo_config
    assert "codegen-backend" not in cargo_config
    assert "-fuse-ld=mold" not in cargo_config
