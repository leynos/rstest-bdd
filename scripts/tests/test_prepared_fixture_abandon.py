"""Unit tests for a timed-out step whose tree cannot be confirmed gone.

``test_prepared_fixture_tree`` covers the cleanup that works. These cases
cover the one that does not: a process that outlives ``SIGKILL`` cannot be
waited for without a bound, so the step has to give up on it and still report
to the driver, which then keeps the target directory. They also cover an
interrupted step, which leaves the same tree behind that a timed-out one does.
"""

import contextlib
import os
import shutil
import signal
import subprocess  # ruff: ignore[suspicious-subprocess-import] - spawns a fixed interpreter argv; no shell.
import sys
import time
import typing as typ
from pathlib import Path

import pytest

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

import check_prepared_fixture_workflow as workflow
import prepared_fixture_step as step
import prepared_fixture_tree as tree
from test_prepared_fixture_tree import POLITE_CHILD, group_is_empty, spawned

if typ.TYPE_CHECKING:
    import collections.abc as cabc

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
    monkeypatch.setattr(tree, "TREE_REAP_SECONDS", 0.5)


@pytest.fixture
def unreapable(
    monkeypatch: pytest.MonkeyPatch,
) -> cabc.Iterator[list[subprocess.Popen[str]]]:
    """Make the termination's own wait time out, as a stuck process would.

    Only ``prepared_fixture_tree.reap`` is replaced, which is the one wait the
    termination makes. Every other wait in the interpreter is left as it is,
    so a step that still waits without a bound blocks on the real process and
    the case's wall-clock bound catches it.

    Yields
    ------
    list[subprocess.Popen[str]]
        Every process whose reap was withheld. The fixture reaps them
        afterwards: they were killed, and only the wait was made to fail.
    """
    leaders: list[subprocess.Popen[str]] = []

    def reap(process: subprocess.Popen[str]) -> None:
        """Report that the process did not exit within the grace period."""
        leaders.append(process)
        raise subprocess.TimeoutExpired(process.args, tree.TREE_REAP_SECONDS)

    monkeypatch.setattr(tree, "reap", reap)
    yield leaders
    for leader in leaders:
        leader.wait()


@pytest.mark.usefixtures("unreapable")
def test_a_leader_that_cannot_be_reaped_is_a_cleanup_failure() -> None:
    """A wait that times out after ``SIGKILL`` is reported, not suppressed."""
    with spawned(POLITE_CHILD) as process, pytest.raises(tree.TreeSurvivedError):
        tree.terminate_tree(process)


@pytest.mark.usefixtures("unreapable")
def test_a_step_with_an_unreapable_leader_fails_promptly(tmp_path: Path) -> None:
    """The cleanup failure replaces the timeout, and nothing waits unbounded."""
    started = time.monotonic()
    with pytest.raises(tree.TreeSurvivedError):
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
        raise tree.TreeSurvivedError(message)

    monkeypatch.setattr(tree, "terminate_tree", terminate_tree)
    try:
        with pytest.raises(tree.TreeSurvivedError):
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


@pytest.fixture
def interrupt_soon() -> cabc.Iterator[None]:
    """Raise ``KeyboardInterrupt`` in this thread half a second from now.

    The interrupt arrives as a signal, the way a real one does, so it lands
    inside whatever the step is blocked in and is handled by that code's own
    interrupt path.
    """

    def interrupt(signum: int, frame: object) -> typ.NoReturn:
        """Turn the alarm into the interrupt under test."""
        del signum, frame
        raise KeyboardInterrupt

    previous = signal.signal(signal.SIGALRM, interrupt)
    signal.setitimer(signal.ITIMER_REAL, 0.5)
    try:
        yield
    finally:
        signal.setitimer(signal.ITIMER_REAL, 0)
        signal.signal(signal.SIGALRM, previous)


@pytest.mark.usefixtures("interrupt_soon")
def test_an_interrupt_after_the_leader_exited_keeps_the_directory(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    """A descendant that outlives a reaped leader is reported, not ignored.

    The step's process exits at once and its descendant holds the captured
    pipe, so the step is still waiting when the interrupt lands. The real
    ``communicate`` reaps the exited leader before it re-raises, which leaves
    a group that can no longer be signalled safely. It must not be taken for
    a group that has gone: the driver would then remove the directory under
    the descendant.
    """
    monkeypatch.setattr(step, "STEP_TIMEOUT_SECONDS", 60)
    pid_file = tmp_path / "pid"
    program = (
        "import os, subprocess, sys\n"
        f"open({str(pid_file)!r}, 'w').write(str(os.getpid()))\n"
        "subprocess.Popen([sys.executable, '-c', 'import time; time.sleep(120)'])\n"
    )
    used: list[Path] = []

    def run_workflow(target_dir: Path) -> None:
        """Run the one step the interrupt lands in."""
        used.append(target_dir)
        step.run_step([sys.executable, "-c", program], target_dir=target_dir)

    monkeypatch.setattr(workflow, "run_workflow", run_workflow)
    try:
        assert workflow.main([]) == 1, "an unconfirmed cleanup is a failure"
        assert used[0].is_dir(), "the directory must survive the interrupt"
    finally:
        # The descendant is this test's own, so its group is safe to signal.
        with contextlib.suppress(ProcessLookupError, FileNotFoundError):
            os.killpg(int(pid_file.read_text(encoding="utf-8")), signal.SIGKILL)
        for directory in used:
            shutil.rmtree(directory, ignore_errors=True)


@pytest.mark.usefixtures("interrupt_soon")
def test_an_interrupted_step_has_its_tree_terminated(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    """An interrupt removes the tree before it propagates.

    The interrupt lands inside the real ``communicate`` while the step's
    process is alive. Left to the default handling, that process would be
    waited for and its descendants left building into a directory the driver
    then removes.
    """
    monkeypatch.setattr(step, "STEP_TIMEOUT_SECONDS", 60)
    pid_file = tmp_path / "pid"
    program = (
        "import os, time\n"
        f"open({str(pid_file)!r}, 'w').write(str(os.getpid()))\n"
        "time.sleep(120)\n"
    )
    with pytest.raises(KeyboardInterrupt):
        step.run_step([sys.executable, "-c", program], target_dir=tmp_path)
    group = int(pid_file.read_text(encoding="utf-8"))
    assert group_is_empty(group), "the interrupted step's tree must be gone"


def test_a_leader_that_cannot_be_reaped_still_reaches_the_driver(
    monkeypatch: pytest.MonkeyPatch, unreapable: list[subprocess.Popen[str]]
) -> None:
    """A process that survives ``SIGKILL`` fails the run without hanging it.

    The real ``run_step`` is driven through ``main`` with the reap withheld.
    The step must give up on the process, not wait for it without a bound, so
    the error reaches the driver and the driver keeps the directory.
    """
    used: list[Path] = []

    def run_workflow(target_dir: Path) -> None:
        """Run one step that outlives its bound."""
        used.append(target_dir)
        step.run_step(SLEEPER, target_dir=target_dir)

    monkeypatch.setattr(workflow, "run_workflow", run_workflow)
    started = time.monotonic()
    try:
        assert workflow.main([]) == 1, "an unreapable leader is a failure"
        assert used[0].is_dir(), "the directory must survive an unconfirmed cleanup"
        elapsed = time.monotonic() - started
        assert elapsed < PROMPT_SECONDS, f"the driver waited {elapsed:.1f} s"
        assert unreapable, "the reap must have been attempted"
    finally:
        for directory in used:
            shutil.rmtree(directory, ignore_errors=True)
