"""Fake cargo/local helpers for publish-check tests.

This module provides small stand-ins for the ``local`` and ``cargo`` helpers
used by the publish-check workflow tests. The fakes record command arguments,
working-directory changes, environment mutations, and delegated runner calls so
tests can assert behaviour without invoking a real subprocess layer.

The main entry points are ``FakeLocal``, which mimics the ``local`` object used
by the publish-check scripts, and ``FakeCargo`` / ``FakeCargoInvocation``,
which turn ``local["cargo"][...]`` lookups into recorded invocations. Use these
helpers when a test needs to assert how cargo would be called or when the
current working directory or environment would be changed.

Examples
--------
>>> calls: list[tuple[list[str], int | None]] = []
>>> local = FakeLocal(lambda args, timeout: (0, "", ""))
>>> _ = local["cargo"]["check", "--tests"].run(retcode=None, timeout=30)
>>> local.invocations
[(['cargo', 'check', '--tests'], 30)]
"""

from __future__ import annotations

import collections.abc as cabc
import contextlib
import typing as typ

RunCallable = cabc.Callable[[list[str], int | None], tuple[int, str, str]]

if typ.TYPE_CHECKING:
    from pathlib import Path


class FakeCargoInvocation:
    """Record a cargo invocation and proxy execution to the fake runner.

    Parameters
    ----------
    local : FakeLocal
        Fake local environment that will service the recorded invocation.
    args : list[str]
        Normalized cargo arguments excluding the ``cargo`` executable name.
    """

    def __init__(self, local: FakeLocal, args: list[str]) -> None:
        """Store the invocation context for later assertions."""
        self._local = local
        self._args = ["cargo", *args]

    def run(
        self, *, retcode: object | None, timeout: int | None
    ) -> tuple[int, str, str]:
        """Record an invocation and delegate to the configured callable.

        Parameters
        ----------
        retcode : object | None
            Unused compatibility argument matching the real interface.
        timeout : int | None
            Timeout recorded alongside the invocation.

        Returns
        -------
        tuple[int, str, str]
            Exit status, stdout, and stderr from the fake runner callable.
        """
        _ = retcode
        self._local.invocations.append((self._args, timeout))
        return self._local.run_callable(self._args, timeout)


class FakeCargo:
    """Proxy indexing calls into ``FakeCargoInvocation`` instances.

    Parameters
    ----------
    local : FakeLocal
        Fake local environment that owns the resulting invocations.
    """

    def __init__(self, local: FakeLocal) -> None:
        """Initialise the cargo proxy for a fake local environment."""
        self._local = local

    def __getitem__(self, args: object) -> FakeCargoInvocation:
        """Return an invocation wrapper for the provided command arguments.

        Parameters
        ----------
        args : object
            Cargo arguments supplied via index access.

        Returns
        -------
        FakeCargoInvocation
            Invocation wrapper that records calls to ``run``.
        """
        match args:
            case list() | tuple() as sequence:
                extras = [str(arg) for arg in sequence]
            case _:
                extras = [str(args)]
        return FakeCargoInvocation(self._local, extras)


class FakeLocal:
    """Mimic a fabric ``local`` helper for cargo orchestration tests.

    Parameters
    ----------
    run_callable : RunCallable
        Callable that returns the fake process result for recorded invocations.

    Notes
    -----
    Instances record ``cwd`` changes, environment mutations, and cargo
    invocations for later assertions.
    """

    def __init__(self, run_callable: RunCallable) -> None:
        """Store the callable that will service fake local invocations."""
        self.run_callable = run_callable
        self.cwd_calls: list[Path] = []
        self.env_calls: list[dict[str, str]] = []
        self.invocations: list[tuple[list[str], int | None]] = []

    def _record_call(
        self, calls: list[object], value: object
    ) -> contextlib.AbstractContextManager[None]:
        """Append ``value`` to ``calls`` and return a null context manager."""
        calls.append(value)
        return contextlib.nullcontext()

    def __getitem__(self, command: str) -> FakeCargo:
        """Return a ``FakeCargo`` proxy for the ``cargo`` command.

        Parameters
        ----------
        command : str
            Command name being requested from the fake local helper.

        Returns
        -------
        FakeCargo
            Cargo proxy used to record later subcommands.

        Raises
        ------
        RuntimeError
            Raised when a command other than ``"cargo"`` is requested.
        """
        match command:
            case "cargo":
                return FakeCargo(self)
            case _:
                msg = (
                    "FakeLocal only understands the 'cargo' command, "
                    f"received {command!r}"
                )
                raise RuntimeError(msg)

    def cwd(self, path: Path) -> contextlib.AbstractContextManager[None]:
        """Record the working directory change for later assertions.

        Parameters
        ----------
        path : Path
            Working directory that the caller wants to enter.

        Yields
        ------
        None
            Null context manager used only for structural compatibility.
        """
        return self._record_call(self.cwd_calls, path)  # type: ignore[arg-type]

    def env(self, **kwargs: str) -> contextlib.AbstractContextManager[None]:
        """Record environment mutations for later assertions.

        Parameters
        ----------
        **kwargs : str
            Environment updates requested by the caller.

        Yields
        ------
        None
            Null context manager used only for structural compatibility.
        """
        return self._record_call(self.env_calls, kwargs)  # type: ignore[arg-type]
