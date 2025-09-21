"""Tests for :mod:`scripts.run_publish_check`."""

from __future__ import annotations

import contextlib
import importlib.util
import io
import os
import subprocess
import sys
import tempfile
import types
import unittest
from pathlib import Path
from unittest import mock

_SCRIPTS_DIR = Path(__file__).resolve().parents[1]
if str(_SCRIPTS_DIR) not in sys.path:
    sys.path.insert(0, str(_SCRIPTS_DIR))

_publish_patch_stub = types.ModuleType("publish_patch")
_publish_patch_stub.REPLACEMENTS = {}


def _noop_apply_replacements(*_args: object, **_kwargs: object) -> None:
    return None


_publish_patch_stub.apply_replacements = _noop_apply_replacements
sys.modules.setdefault("publish_patch", _publish_patch_stub)

_MODULE_PATH = _SCRIPTS_DIR / "run_publish_check.py"
_SPEC = importlib.util.spec_from_file_location("run_publish_check", _MODULE_PATH)
_run_publish_check = importlib.util.module_from_spec(_SPEC)
assert _SPEC and _SPEC.loader  # narrow mypy and silence None analysis
_SPEC.loader.exec_module(_run_publish_check)

run_cargo_command = _run_publish_check.run_cargo_command


class RunCargoCommandUnitTests(unittest.TestCase):
    """Unit tests that exercise validation paths in :func:`run_cargo_command`."""

    def setUp(self) -> None:  # noqa: D401 - unittest API contract
        self._workspace = tempfile.TemporaryDirectory()
        self.addCleanup(self._workspace.cleanup)
        self.workspace = Path(self._workspace.name)
        (self.workspace / "crates" / "demo").mkdir(parents=True)

    def test_rejects_non_cargo_command(self) -> None:
        """``run_cargo_command`` requires the command to start with ``cargo``."""

        with self.assertRaises(ValueError):
            run_cargo_command("demo", self.workspace, ["not-cargo"])

    def test_invalid_timeout_logs_and_exits(self) -> None:
        """An invalid timeout emits an error log before exiting."""

        with mock.patch.dict(os.environ, {"PUBLISH_CHECK_TIMEOUT_SECS": "oops"}):
            with self.assertLogs(level="ERROR") as captured:
                with self.assertRaises(SystemExit):
                    run_cargo_command("demo", self.workspace, ["cargo", "--version"])
        self.assertTrue(
            any("PUBLISH_CHECK_TIMEOUT_SECS" in entry for entry in captured.output)
        )

    def test_logs_output_when_command_fails(self) -> None:
        """Failures surface the captured stdout and stderr in the log."""

        failure = subprocess.CompletedProcess(
            args=["cargo", "check"],
            returncode=1,
            stdout="compile error",
            stderr="missing dependency",
        )
        with mock.patch.dict(os.environ, {}, clear=True), mock.patch(
            "run_publish_check.subprocess.run", return_value=failure
        ) as run_mock, self.assertLogs(level="ERROR") as captured:
            with self.assertRaises(subprocess.CalledProcessError):
                run_cargo_command("demo", self.workspace, ["cargo", "check"])
        run_mock.assert_called_once()
        joined_logs = "\n".join(captured.output)
        self.assertIn("cargo stdout", joined_logs)
        self.assertIn("cargo stderr", joined_logs)


class RunCargoCommandBehaviouralTests(unittest.TestCase):
    """Behavioural tests that execute ``cargo`` in a throwaway workspace."""

    def test_runs_cargo_command_successfully(self) -> None:
        """The helper executes a cargo command and streams its output."""

        with tempfile.TemporaryDirectory() as tmp:
            workspace = Path(tmp)
            crate_dir = workspace / "crates" / "demo"
            crate_dir.mkdir(parents=True)
            stdout_buffer = io.StringIO()
            stderr_buffer = io.StringIO()
            with mock.patch.dict(os.environ, {}, clear=True), contextlib.redirect_stdout(
                stdout_buffer
            ), contextlib.redirect_stderr(stderr_buffer):
                run_cargo_command("demo", workspace, ["cargo", "--version"])
            output = stdout_buffer.getvalue()
            self.assertIn("cargo", output.lower())
            self.assertEqual("", stderr_buffer.getvalue())


if __name__ == "__main__":
    unittest.main()
