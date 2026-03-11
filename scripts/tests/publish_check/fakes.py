"""Fake cargo/local helpers for publish-check tests."""

from __future__ import annotations

import contextlib
import typing as typ

RunCallable = typ.Callable[[list[str], int | None], tuple[int, str, str]]

if typ.TYPE_CHECKING:
    from pathlib import Path


class FakeCargoInvocation:
    """Record a cargo invocation and proxy execution to the fake runner."""

    def __init__(self, local: FakeLocal, args: list[str]) -> None:
        """Store the invocation context for later assertions."""
        self._local = local
        self._args = ["cargo", *args]

    def run(
        self, *, retcode: object | None, timeout: int | None
    ) -> tuple[int, str, str]:
        """Record an invocation and delegate to the configured callable."""
        self._local.invocations.append((self._args, timeout))
        return self._local.run_callable(self._args, timeout)


class FakeCargo:
    """Proxy indexing calls into ``FakeCargoInvocation`` instances."""

    def __init__(self, local: FakeLocal) -> None:
        """Initialise the cargo proxy for a fake local environment."""
        self._local = local

    def __getitem__(self, args: object) -> FakeCargoInvocation:
        """Return an invocation wrapper for the provided command arguments."""
        extras = list(args) if isinstance(args, (list, tuple)) else [str(args)]
        return FakeCargoInvocation(self._local, extras)


class FakeLocal:
    """Mimic a fabric ``local`` helper for cargo orchestration tests."""

    def __init__(self, run_callable: RunCallable) -> None:
        """Store the callable that will service fake local invocations."""
        self.run_callable = run_callable
        self.cwd_calls: list[Path] = []
        self.env_calls: list[dict[str, str]] = []
        self.invocations: list[tuple[list[str], int | None]] = []

    def __getitem__(self, command: str) -> FakeCargo:
        """Return a ``FakeCargo`` proxy for the ``cargo`` command."""
        if command != "cargo":
            msg = (
                f"FakeLocal only understands the 'cargo' command, received {command!r}"
            )
            raise RuntimeError(msg)
        return FakeCargo(self)

    def cwd(self, path: Path) -> contextlib.AbstractContextManager[None]:
        """Record the working directory change for later assertions."""
        self.cwd_calls.append(path)
        return contextlib.nullcontext()

    def env(self, **kwargs: str) -> contextlib.AbstractContextManager[None]:
        """Record environment mutations for later assertions."""
        self.env_calls.append(kwargs)
        return contextlib.nullcontext()
