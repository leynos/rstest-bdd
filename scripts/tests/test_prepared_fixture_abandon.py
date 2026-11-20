"""Unit tests for a timed-out step whose tree cannot be confirmed gone.

``test_prepared_fixture_tree`` covers the cleanup that works. These cases
cover the one that does not: a process that outlives ``SIGKILL`` cannot be
waited for without a bound, so the step has to give up on it and still report
to the driver, which then keeps the target directory. They also cover an
interrupted step, which leaves the same tree behind that a timed-out one does.
"""

import contextlib
import os
import signal
import subprocess  # ruff: ignore[suspicious-subprocess-import] - spawns a fixed interpreter argv; no shell.
import sys
import time
import typing as typ
from pathlib import Path

import pytest

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

import prepared_fixture_step as step
from test_prepared_fixture_tree import POLITE_CHILD, group_is_empty, spawned

if typ.TYPE_CHECKING:
    import collections.abc as cabc

pytestmark = pytest.mark.skipif(os.name != "posix", reason="needs process groups")

#: A step that would run far past any bound a case sets.
SLEEPER = [sys.executable, "-c", "import time\ntime.sleep(120)\n"]

#: The longest a case may take when no wait is unbounded.
#:
#: The shortened step timeout and grace period sum to well under a second; the
#: sleeper runs for two minutes, so an unbounded wait cannot hide inside this.
PROMPT_SECONDS = 20.0


@pytest.fixture(autouse=True)
def short_bounds(monkeypatch: pytest.MonkeyPatch) -> None:
    """Shorten the step timeout and the grace period for every case here."""
    monkeypatch.setattr(step, "STEP_TIMEOUT_SECONDS", 0.5)
    monkeypatch.setattr(step, "TREE_REAP_SECONDS", 0.5)


@pytest.fixture
def unreapable(
    monkeypatch: pytest.MonkeyPatch,
) -> cabc.Iterator[list[subprocess.Popen[str]]]:
    """Make every bounded wait for a process time out, as a stuck one would.

    An unbounded wait is passed through, so a step that still makes one blocks
    on the real process and the case's wall-clock bound catches it.

    Yields
    ------
    list[subprocess.Popen[str]]
        Every process a bounded wait was attempted on, which the fixture reaps
        afterwards: the processes were killed, and only the wait was withheld.
    """
    real_wait = subprocess.Popen.wait
    leaders: list[subprocess.Popen[str]] = []

    def wait(self: subprocess.Popen[str], timeout: float | None = None) -> int:
        """Time out a bounded wait; let an unbounded one through."""
        if timeout is None:
            return real_wait(self)
        leaders.append(self)
        raise subprocess.TimeoutExpired(self.args, timeout)

    monkeypatch.setattr(subprocess.Popen, "wait", wait)
    yield leaders
    for leader in leaders:
        real_wait(leader)


@pytest.mark.usefixtures("unreapable")
def test_a_leader_that_cannot_be_reaped_is_a_cleanup_failure() -> None:
    """A wait that times out after ``SIGKILL`` is reported, not suppressed."""
    with spawned(POLITE_CHILD) as process, pytest.raises(step.TreeSurvivedError):
        step.terminate_tree(process)


@pytest.mark.usefixtures("unreapable")
def test_a_step_with_an_unreapable_leader_fails_promptly(tmp_path: Path) -> None:
    """The cleanup failure replaces the timeout, and nothing waits unbounded."""
    started = time.monotonic()
    with pytest.raises(step.TreeSurvivedError):
        step.run_step(SLEEPER, target_dir=tmp_path)
    elapsed = time.monotonic() - started
    assert elapsed < PROMPT_SECONDS, f"the step waited {elapsed:.1f} s"


def test_a_surviving_tree_is_abandoned_while_it_is_still_running(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    """The step raises with the process alive and its pipes released.

    Termination is replaced by a failure that kills nothing, so the leader is
    genuinely still running when the step gives up. Waiting for it then would
    block for the rest of its two minutes.
    """
    survivors: list[subprocess.Popen[str]] = []

    def terminate_tree(process: subprocess.Popen[str]) -> None:
        """Fail the cleanup without touching the process."""
        survivors.append(process)
        message = f"process group {process.pid} still has a member"
        raise step.TreeSurvivedError(message)

    monkeypatch.setattr(step, "terminate_tree", terminate_tree)
    try:
        with pytest.raises(step.TreeSurvivedError):
            step.run_step(SLEEPER, target_dir=tmp_path)
        leader = survivors[0]
        assert leader.poll() is None, "the leader must not have been waited for"
        released = leader.stdout is not None and leader.stdout.closed
        assert released, "the abandoned process's pipes must be closed"
    finally:
        for survivor in survivors:
            with contextlib.suppress(ProcessLookupError):
                os.killpg(survivor.pid, signal.SIGKILL)
            survivor.wait()


def test_an_interrupted_step_has_its_tree_terminated(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    """An interrupt removes the tree before it propagates.

    Left to the default handling, the step's process would be waited for and
    its descendants left building into a directory the driver then removes.
    """
    leaders: list[subprocess.Popen[str]] = []

    def communicate(
        self: subprocess.Popen[str], timeout: float | None = None
    ) -> tuple[str, str]:
        """Interrupt the step as soon as it starts waiting for its process."""
        del timeout
        leaders.append(self)
        raise KeyboardInterrupt

    monkeypatch.setattr(subprocess.Popen, "communicate", communicate)
    with pytest.raises(KeyboardInterrupt):
        step.run_step(SLEEPER, target_dir=tmp_path)
    leader = leaders[0]
    assert leader.returncode is not None, "the leader must have been reaped"
    assert group_is_empty(leader.pid), "the interrupted step's tree must be gone"
