"""Integration coverage for the opt-in dev-fast Makefile targets."""

from __future__ import annotations

import dataclasses as dc
import ntpath
import os
import re
import shlex
import shutil
import stat
import subprocess  # noqa: S404 - integration test invokes the trusted local Makefile.
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
    fake_cargo_reference: str
    invocation_log: Path
    result: subprocess.CompletedProcess[str]
    real_cargo: str | None


@dc.dataclass(frozen=True)
class RecipeShellEnvironment:
    """The Make executable and shell that evaluate a target recipe."""

    make_executable: str
    recipe_shell: str
    cygpath: str | None


def make_executable() -> str:
    """Return the absolute Make executable used by integration tests."""
    executable = shutil.which("make")
    assert executable is not None, "make executable should be available"
    return executable


def recipe_shell_environment() -> RecipeShellEnvironment:
    """Discover the shell that the selected Make uses for its recipes."""
    executable = make_executable()
    result = subprocess.run(  # noqa: S603 - the local Make executable is trusted.
        [executable, "--no-print-directory", "-f", "-", "print-recipe-shell"],
        cwd=REPO_ROOT,
        input='print-recipe-shell:\n\t@printf "%s\\n" "$(SHELL)"\n',
        text=True,
        capture_output=True,
        check=False,
        timeout=30,
    )
    assert result.returncode == 0, result.stderr
    return RecipeShellEnvironment(
        make_executable=executable,
        recipe_shell=result.stdout.strip(),
        cygpath=shutil.which("cygpath"),
    )


def is_posix_hosted_windows_shell(environment: RecipeShellEnvironment) -> bool:
    """Return whether MinGW/MSYS Make delegates recipes to a POSIX shell."""
    return environment.make_executable.lower().endswith(
        ".exe"
    ) and environment.recipe_shell.startswith("/")


def is_native_windows_path(path: str) -> bool:
    """Return whether *path* uses a drive-qualified Windows path syntax."""
    drive, _ = ntpath.splitdrive(path)
    return bool(drive)


def cygpath_unix(path: str, cygpath: str) -> str:
    """Convert a Windows path to the active POSIX shell namespace."""
    result = subprocess.run(  # noqa: S603 - cygpath comes from the active shell environment.
        [cygpath, "--unix", path],
        text=True,
        capture_output=True,
        check=False,
        timeout=30,
    )
    assert result.returncode == 0, result.stderr
    converted = result.stdout.strip()
    assert converted, "cygpath should return a POSIX path"
    return converted


def msys_unix_path(path: str) -> str:
    """Map a drive-qualified Windows path to MSYS/MinGW's `/drive` namespace."""
    drive, tail = ntpath.splitdrive(path)
    assert drive, f"expected a drive-qualified Windows path, got {path!r}"
    unix_tail = tail.replace("\\", "/").lstrip("/")
    return f"/{drive[0].lower()}/{unix_tail}"


def shell_safe_executable_reference(
    path: Path,
    environment: RecipeShellEnvironment,
) -> str:
    """Return the unquoted executable reference understood by Make's shell."""
    native_path = str(path)
    if not (
        is_posix_hosted_windows_shell(environment)
        and is_native_windows_path(native_path)
    ):
        return native_path
    if environment.cygpath is not None:
        return cygpath_unix(native_path, environment.cygpath)
    return msys_unix_path(native_path)


def shell_safe_command_word(reference: str) -> str:
    """Quote an executable reference only when the POSIX shell requires it."""
    return shlex.quote(reference)


def write_fake_cargo(tmp_path: Path) -> tuple[Path, Path]:
    """Create a Cargo stand-in that records each received argument vector."""
    invocation_log = tmp_path / "cargo-invocations.log"
    fake_cargo = tmp_path / "cargo"
    fake_cargo.write_text(
        "\n".join([
            "#!/usr/bin/env sh",
            "",
            'printf "%s" "$0" >> "$FAKE_CARGO_INVOCATIONS"',
            'for argument in "$@"; do',
            '    printf "\\0%s" "$argument" >> "$FAKE_CARGO_INVOCATIONS"',
            "done",
            'printf "\\n" >> "$FAKE_CARGO_INVOCATIONS"',
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
    environment = recipe_shell_environment()
    fake_cargo_reference = shell_safe_executable_reference(fake_cargo, environment)
    invocation_log_reference = shell_safe_executable_reference(
        invocation_log,
        environment,
    )
    original_path = os.environ["PATH"]
    real_cargo = shutil.which("cargo", path=original_path)
    monkeypatch.setenv("PATH", f"{tmp_path}{os.pathsep}{original_path}")
    monkeypatch.setenv("CARGO", shell_safe_command_word(fake_cargo_reference))
    monkeypatch.setenv("FAKE_CARGO_INVOCATIONS", invocation_log_reference)
    result = subprocess.run(  # noqa: S603 - target and command are controlled by this test.
        [environment.make_executable, "--no-print-directory", target],
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
        fake_cargo_reference=fake_cargo_reference,
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
    assert executable == invocation.fake_cargo_reference
    if invocation.real_cargo is not None:
        assert executable != invocation.real_cargo
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


def test_shell_safe_executable_reference_preserves_a_posix_path() -> None:
    """Unix-like Make environments retain a directly executable path."""
    environment = RecipeShellEnvironment(
        make_executable="/usr/bin/make",
        recipe_shell="/bin/sh",
        cygpath=None,
    )

    assert (
        shell_safe_executable_reference(Path("/example/fake-cargo"), environment)
        == "/example/fake-cargo"
    )


def test_shell_safe_executable_reference_uses_cygpath_for_mingw(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """MinGW's POSIX recipe shell receives the path returned by `cygpath`."""
    environment = RecipeShellEnvironment(
        make_executable=r"C:\mingw64\bin\make.EXE",
        recipe_shell="/usr/bin/sh",
        cygpath=r"C:\msys64\usr\bin\cygpath.exe",
    )
    observed_command: list[str] = []

    def fake_run(command: list[str], **_: object) -> subprocess.CompletedProcess[str]:
        observed_command.extend(command)
        return subprocess.CompletedProcess(
            command,
            0,
            "/c/Users/runneradmin/cargo\n",
            "",
        )

    monkeypatch.setattr(subprocess, "run", fake_run)

    assert (
        shell_safe_executable_reference(
            Path(r"C:\Users\runneradmin\cargo"),
            environment,
        )
        == "/c/Users/runneradmin/cargo"
    )
    assert observed_command == [
        r"C:\msys64\usr\bin\cygpath.exe",
        "--unix",
        r"C:\Users\runneradmin\cargo",
    ]


def test_shell_safe_executable_reference_uses_msys_fallback_without_cygpath() -> None:
    """A MinGW shell without `cygpath` receives the equivalent MSYS path."""
    environment = RecipeShellEnvironment(
        make_executable=r"C:\mingw64\bin\make.EXE",
        recipe_shell="/usr/bin/sh",
        cygpath=None,
    )

    assert (
        shell_safe_executable_reference(
            Path(r"C:\Users\runneradmin\cargo"),
            environment,
        )
        == "/c/Users/runneradmin/cargo"
    )
