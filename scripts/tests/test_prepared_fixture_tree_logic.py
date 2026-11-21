"""Unit tests for the rules of process-tree termination, on every platform.

``test_prepared_fixture_tree`` proves the termination against real process
groups, which only POSIX has. The *rules* do not need a real group: which
signals are sent, in what order relative to the reap, and what is reported
when a tree cannot be confirmed gone. These cases state those rules against a
stand-in process and a recorded operating system, so they run wherever the
scripts do and fail on the rule, not on the platform.
"""

import contextlib
import dataclasses
import io
import os
import signal
import subprocess  # ruff: ignore[suspicious-subprocess-import] - only raises TimeoutExpired; spawns nothing.
import sys
import typing as typ
from pathlib import Path

import pytest

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

import prepared_fixture_tree as tree

#: The stand-in group's id, and the value used for ``SIGKILL`` where the
#: platform does not define one.
GROUP = 4242
KILL = 9


@dataclasses.dataclass(slots=True)
class Leader:
    """A stand-in for the step's process.

    Attributes
    ----------
    returncode : int | None
        ``None`` until the process is reaped, as on a real ``Popen``.
    reapable : bool
        Whether a bounded wait succeeds. A process stuck in the kernel is one
        whose wait always times out.
    pid : int
        The process id, which is also its group's id.
    stdout, stderr : io.StringIO
        The captured pipes.
    """

    returncode: int | None = None
    reapable: bool = True
    pid: int = GROUP
    stdout: io.StringIO = dataclasses.field(default_factory=io.StringIO)
    stderr: io.StringIO = dataclasses.field(default_factory=io.StringIO)

    def wait(self, timeout: float | None = None) -> int:
        """Reap the process, or time out if it cannot be reaped.

        Returns
        -------
        int
            The status the process was reaped with.

        Raises
        ------
        subprocess.TimeoutExpired
            When the process is not reapable.
        """
        if not self.reapable:
            raise subprocess.TimeoutExpired(["step"], timeout or 0.0)
        self.returncode = -KILL
        return self.returncode


@dataclasses.dataclass(slots=True)
class System:
    """A recorded operating system for one stand-in group.

    Attributes
    ----------
    survivors : bool
        Whether the group still has a member once it has been killed.
    sent : list[tuple[int, bool]]
        One ``(signal, unreaped)`` pair per call to ``killpg``, in order.
    leader : Leader | None
        The process whose reaping the record tracks.
    """

    survivors: bool = False
    sent: list[tuple[int, bool]] = dataclasses.field(default_factory=list)
    leader: Leader | None = None

    def killpg(self, group: int, sig: int) -> None:
        """Record *sig*; answer signal zero from whether the group survives.

        Raises
        ------
        ProcessLookupError
            For signal zero, when no member of the group is left.
        """
        del group
        unreaped = self.leader is None or self.leader.returncode is None
        self.sent.append((sig, unreaped))
        if sig == 0 and not self.survivors:
            raise ProcessLookupError

    def delivered(self) -> list[tuple[int, bool]]:
        """Return the signals that could reach a process, without the probes."""
        return [(sig, unreaped) for sig, unreaped in self.sent if sig != 0]


@pytest.fixture
def system(monkeypatch: pytest.MonkeyPatch) -> System:
    """Replace the operating-system calls the termination makes.

    Every name is set whether or not the platform defines it, which is what
    lets these cases run where process groups do not exist.

    Returns
    -------
    System
        The record a case inspects.
    """
    recorded = System()
    monkeypatch.setattr(os, "killpg", recorded.killpg, raising=False)
    monkeypatch.setattr(os, "waitid", lambda *_args: object(), raising=False)
    for name in ("P_PID", "WEXITED", "WNOHANG", "WNOWAIT"):
        monkeypatch.setattr(os, name, 0, raising=False)
    monkeypatch.setattr(signal, "SIGKILL", KILL, raising=False)
    monkeypatch.setattr(tree, "TREE_REAP_SECONDS", 0.05)
    monkeypatch.setattr(tree, "GROUP_POLL_SECONDS", 0.01)
    return recorded


def as_process(leader: Leader) -> subprocess.Popen[str]:
    """Hand *leader* to code that is typed for a real process."""
    return typ.cast("subprocess.Popen[str]", leader)


def test_both_signals_are_sent_before_the_reap_and_only_probes_after(
    system: System,
) -> None:
    """``SIGTERM`` then ``SIGKILL`` while unreaped; signal zero afterwards."""
    system.leader = leader = Leader()
    tree.terminate_tree(as_process(leader))
    expected = [(signal.SIGTERM, True), (KILL, True)]
    assert system.delivered() == expected, "the signals or their order drifted"
    probes = [unreaped for sig, unreaped in system.sent if sig == 0]
    assert probes == [False], "the group is probed once, after the reap"
    assert leader.returncode is not None, "the leader must be reaped"


def test_a_group_with_a_survivor_after_sigkill_is_reported(system: System) -> None:
    """A group that will not empty is a cleanup failure naming the group."""
    system.survivors = True
    system.leader = leader = Leader()
    with pytest.raises(tree.TreeSurvivedError, match=str(GROUP)):
        tree.terminate_tree(as_process(leader))


def test_a_leader_that_will_not_be_reaped_is_reported(system: System) -> None:
    """A wait that times out after ``SIGKILL`` is not suppressed."""
    system.leader = leader = Leader(reapable=False)
    with pytest.raises(tree.TreeSurvivedError, match="did not exit"):
        tree.terminate_tree(as_process(leader))
    assert leader.returncode is None, "the leader was not reaped"


@pytest.mark.parametrize(
    ("survivors", "raised"),
    [
        pytest.param(False, False, id="its group has emptied"),
        pytest.param(True, True, id="its group still has a member"),
    ],
)
def test_a_reaped_leaders_group_is_probed_and_never_signalled(
    system: System, *, survivors: bool, raised: bool
) -> None:
    """Once reaped, the id may be reused, so only signal zero is sent.

    An empty group is a finished tree. A group that still exists cannot be
    signalled safely and cannot be assumed gone, so it is reported.
    """
    system.survivors = survivors
    system.leader = leader = Leader(returncode=0)
    if raised:
        with pytest.raises(tree.TreeSurvivedError, match="already reaped"):
            tree.terminate_tree(as_process(leader))
    else:
        tree.terminate_tree(as_process(leader))
    assert not system.delivered(), "a reaped leader's group must not be signalled"
    assert system.sent, "the group must still be asked whether it exists"


def test_waiting_for_the_leader_stops_at_the_grace_period(
    monkeypatch: pytest.MonkeyPatch, system: System
) -> None:
    """A leader that never exits does not hold the escalation up for ever."""
    del system
    asked: list[int] = []

    def waitid(_kind: int, pid: int, _flags: int) -> None:
        """Report that the process has not exited."""
        asked.append(pid)

    monkeypatch.setattr(os, "waitid", waitid, raising=False)
    tree.await_leader_exit(GROUP)
    assert asked, "the leader must be asked whether it has exited"
    assert set(asked) == {GROUP}, "only the leader is asked"


class Owned:
    """A context manager that records whether its block was left normally."""

    def __init__(self) -> None:
        """Start outside the block."""
        self.exited = False

    def __enter__(self) -> typ.Self:
        """Enter the block."""
        return self

    def __exit__(self, *_exc: object) -> None:
        """Record that the block was left through the stack."""
        self.exited = True


def test_a_confirmed_cleanup_leaves_the_process_in_its_stack(system: System) -> None:
    """A tree that has gone is left for the stack to close as usual."""
    system.leader = leader = Leader()
    owned = Owned()
    with contextlib.ExitStack() as stack:
        stack.enter_context(owned)
        tree.settle(stack, as_process(leader))
    assert owned.exited, "a settled process is still closed by its stack"
    assert not leader.stdout.closed, "its pipes are the stack's to close"


def test_an_unconfirmed_cleanup_abandons_the_process(system: System) -> None:
    """A tree that survived is taken out of the stack with its pipes closed.

    Leaving it in the stack would wait for it without a bound when the block
    ends, and the error would never reach the caller.
    """
    system.survivors = True
    system.leader = leader = Leader()
    owned = Owned()
    with contextlib.ExitStack() as stack:
        stack.enter_context(owned)
        with pytest.raises(tree.TreeSurvivedError):
            tree.settle(stack, as_process(leader))
    assert not owned.exited, "an abandoned process must not be waited for"
    closed = leader.stdout.closed and leader.stderr.closed
    assert closed, "an abandoned process's pipes must be closed"


def test_abandoning_a_process_without_pipes_is_harmless() -> None:
    """A process started without captured pipes has none to close."""
    leader = Leader()
    leader.stdout = typ.cast("io.StringIO", None)
    leader.stderr = typ.cast("io.StringIO", None)
    tree.abandon_pipes(as_process(leader))
    assert leader.stdout is None, "there was nothing to close"
