"""CLI wiring tests for the publish-check entry point."""

from __future__ import annotations

import typing as typ

if typ.TYPE_CHECKING:
    from types import ModuleType

    import pytest


def test_main_uses_defaults(
    monkeypatch: pytest.MonkeyPatch,
    run_publish_check_module: ModuleType,
) -> None:
    """Confirm the CLI invokes the workflow with default arguments."""
    captured: dict[str, object] = {}

    def fake_run_publish_check(
        *,
        keep_tmp: bool,
        timeout_secs: int,
        live: bool,
    ) -> None:
        captured["keep_tmp"] = keep_tmp
        captured["timeout_secs"] = timeout_secs
        captured["live"] = live

    monkeypatch.setattr(
        run_publish_check_module, "run_publish_check", fake_run_publish_check
    )

    run_publish_check_module.app([])

    assert captured == {
        "keep_tmp": False,
        "timeout_secs": run_publish_check_module.DEFAULT_PUBLISH_TIMEOUT_SECS,
        "live": False,
    }


def test_main_honours_environment(
    monkeypatch: pytest.MonkeyPatch,
    run_publish_check_module: ModuleType,
) -> None:
    """Ensure environment variables influence the invoked workflow."""
    observed: list[tuple[bool, int, bool]] = []

    def fake_run_publish_check(
        *,
        keep_tmp: bool,
        timeout_secs: int,
        live: bool,
    ) -> None:
        observed.append((keep_tmp, timeout_secs, live))

    monkeypatch.setattr(
        run_publish_check_module, "run_publish_check", fake_run_publish_check
    )
    monkeypatch.setenv("PUBLISH_CHECK_KEEP_TMP", "true")
    monkeypatch.setenv("PUBLISH_CHECK_TIMEOUT_SECS", "60")

    run_publish_check_module.app([])

    assert observed == [(True, 60, False)]


def test_main_cli_overrides_env(
    monkeypatch: pytest.MonkeyPatch,
    run_publish_check_module: ModuleType,
) -> None:
    """Ensure CLI arguments override environment configuration."""
    observed: list[tuple[bool, int, bool]] = []

    def fake_run_publish_check(
        *,
        keep_tmp: bool,
        timeout_secs: int,
        live: bool,
    ) -> None:
        observed.append((keep_tmp, timeout_secs, live))

    monkeypatch.setattr(
        run_publish_check_module, "run_publish_check", fake_run_publish_check
    )
    monkeypatch.setenv("PUBLISH_CHECK_KEEP_TMP", "false")
    monkeypatch.setenv("PUBLISH_CHECK_TIMEOUT_SECS", "900")

    run_publish_check_module.app(["--keep-tmp", "--timeout-secs", "5"])

    assert observed == [(True, 5, False)]


def test_main_live_flag(
    monkeypatch: pytest.MonkeyPatch,
    run_publish_check_module: ModuleType,
) -> None:
    """Verify the live flag toggles the publish workflow accordingly."""
    observed: list[tuple[bool, int, bool]] = []

    def fake_run_publish_check(
        *,
        keep_tmp: bool,
        timeout_secs: int,
        live: bool,
    ) -> None:
        observed.append((keep_tmp, timeout_secs, live))

    monkeypatch.setattr(
        run_publish_check_module, "run_publish_check", fake_run_publish_check
    )

    run_publish_check_module.app(["--live"])

    assert observed == [
        (
            False,
            run_publish_check_module.DEFAULT_PUBLISH_TIMEOUT_SECS,
            True,
        )
    ]
