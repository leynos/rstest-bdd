"""Unit tests for the prepared-fixture step's process-tree termination.

A step that outlives its bound has to be gone, with every descendant, before
the driver removes the target directory it was writing to. These cases drive
``terminate_tree`` and the timeout path of ``run_step`` against small child
processes: a tree that honours ``SIGTERM``, one that ignores it, a descendant
that outlives its parent, and a descendant that holds the captured pipe of a
parent that already exited. They signal process groups, so ``conftest`` leaves
this module uncollected where the platform has none;
``test_prepared_fixture_tree_logic`` covers the same rules there.
"""

import contextlib
import dataclasses
import os
import signal
import subprocess  # ruff: ignore[suspicious-subprocess-import] - spawns a fixed interpreter argv; no shell.
import sys
import typing as typ
from pathlib import Path

import pytest

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

import prepared_fixture_step as step
import prepared_fixture_tree as tree

if typ.TYPE_CHECKING:
    import collections.abc as cabc

#: A child that ignores ``SIGTERM``, says so, and then outlives any test.
#:
#: It announces itself only after the handler is installed, so a test that
#: waits for the line cannot signal a child still able to die politely.
STUBBORN_CHILD = (
    "import signal, sys, time\n"
    "signal.signal(signal.SIGTERM, signal.SIG_IGN)\n"
    "print('ready', flush=True)\n"
    "time.sleep(120)\n"
)

#: A child that keeps the default ``SIGTERM`` disposition.
POLITE_CHILD = "import time\nprint('ready', flush=True)\ntime.sleep(120)\n"


@contextlib.contextmanager
def spawned(program: str) -> cabc.Iterator[subprocess.Popen[str]]:
    """Run *program* in a process group of its own for the length of a block.

    The child has printed its ``ready`` line by the time the block starts, and
    is killed and reaped when the block ends, whatever the case did to it.

    Yields
    ------
    subprocess.Popen[str]
        The running child.
    """
    with subprocess.Popen(  # ruff: ignore[subprocess-without-shell-equals-true] - fixed argv, no shell.
        [sys.executable, "-c", program],
        stdout=subprocess.PIPE,
        text=True,
        start_new_session=True,
    ) as process:
        try:
            assert process.stdout is not None, "the child must have a stdout pipe"
            ready = process.stdout.readline().strip()
            assert ready == "ready", "the child must announce itself"
            yield process
        finally:
            # The whole group, so a descendant a failing case left behind does
            # not outlive the test session.
            with contextlib.suppress(ProcessLookupError):
                os.killpg(process.pid, signal.SIGKILL)


#: A parent that dies on ``SIGTERM`` above a descendant that ignores it.
#:
#: It announces itself only once the descendant has, so both are in place.
PARENT_OF_STUBBORN_CHILD = (
    "import subprocess, sys, time\n"
    f"child = subprocess.Popen([sys.executable, '-c', {STUBBORN_CHILD!r}],"
    " stdout=subprocess.PIPE, text=True)\n"
    "child.stdout.readline()\n"
    "print('ready', flush=True)\n"
    "time.sleep(120)\n"
)

#: A parent and a descendant that both ignore ``SIGTERM``.
#:
#: Only a ``SIGKILL`` sent to the group ends this tree, so a termination that
#: signalled the parent alone would leave the descendant behind.
STUBBORN_TREE = (
    "import signal, subprocess, sys, time\n"
    "signal.signal(signal.SIGTERM, signal.SIG_IGN)\n"
    f"child = subprocess.Popen([sys.executable, '-c', {STUBBORN_CHILD!r}],"
    " stdout=subprocess.PIPE, text=True)\n"
    "child.stdout.readline()\n"
    "print('ready', flush=True)\n"
    "time.sleep(120)\n"
)


def group_is_empty(group: int) -> bool:
    """Return whether no process is left in *group*."""
    try:
        os.killpg(group, 0)
    except ProcessLookupError:
        return True
    return False


@pytest.fixture
def short_reap(monkeypatch: pytest.MonkeyPatch) -> None:
    """Shorten the grace period so a stubborn child costs a moment, not 10 s."""
    monkeypatch.setattr(tree, "TREE_REAP_SECONDS", 0.5)


@pytest.mark.usefixtures("short_reap")
def test_a_tree_that_ignores_sigterm_is_killed_within_the_grace_period() -> None:
    """``SIGKILL`` follows when the polite signal is ignored."""
    with spawned(STUBBORN_CHILD) as process:
        tree.terminate_tree(process)
        killed = process.returncode == -signal.SIGKILL
        assert killed, f"expected SIGKILL, got status {process.returncode}"


@pytest.mark.usefixtures("short_reap")
def test_a_tree_that_honours_sigterm_is_not_escalated() -> None:
    """A child that exits on ``SIGTERM`` is never sent ``SIGKILL``."""
    with spawned(POLITE_CHILD) as process:
        tree.terminate_tree(process)
        polite = process.returncode == -signal.SIGTERM
        assert polite, f"expected SIGTERM, got status {process.returncode}"


@dataclasses.dataclass(slots=True)
class SignalLog:
    """Every signal sent to a process group while a case ran.

    Attributes
    ----------
    sent : list[tuple[int, bool]]
        One ``(signal, unreaped)`` pair per call, in order. ``unreaped`` says
        whether every watched leader still held its id when the signal was
        sent. Signal zero is recorded too; it only asks whether a group exists.
    watched : list[subprocess.Popen[str]]
        The leaders whose reaping the log tracks.
    """

    sent: list[tuple[int, bool]] = dataclasses.field(default_factory=list)
    watched: list[subprocess.Popen[str]] = dataclasses.field(default_factory=list)

    def note(self, sig: int) -> None:
        """Record *sig* and whether every watched leader is still unreaped."""
        unreaped = all(leader.returncode is None for leader in self.watched)
        self.sent.append((sig, unreaped))


@pytest.fixture
def signal_log(monkeypatch: pytest.MonkeyPatch) -> SignalLog:
    """Record every signal the code under test sends to a process group.

    Returns
    -------
    SignalLog
        The log, which a case fills by running the code under test.
    """
    log = SignalLog()
    real_killpg = os.killpg

    def killpg(group: int, sig: int) -> None:
        """Note the signal, then send it."""
        log.note(sig)
        real_killpg(group, sig)

    monkeypatch.setattr(os, "killpg", killpg)
    return log


def test_a_finished_process_is_left_alone(signal_log: SignalLog) -> None:
    """The group of an already-reaped process is asked about, never signalled.

    Its id is no longer tied to this step: the group may have emptied and the
    id been handed to an unrelated process, which a signal would then reach.
    Signal zero only asks whether the group exists, so it is the one thing
    that may still be sent.
    """
    with subprocess.Popen(
        [sys.executable, "-c", "pass"], start_new_session=True, text=True
    ) as process:
        process.wait()
        tree.terminate_tree(process)
        assert process.returncode == 0, "a finished process keeps its status"
    delivered = [sig for sig, _unreaped in signal_log.sent if sig != 0]
    assert not delivered, "a reaped process's group must not be signalled"


@pytest.mark.usefixtures("short_reap")
def test_every_delivered_signal_is_sent_before_the_leader_is_reaped(
    signal_log: SignalLog,
) -> None:
    """The group id is only signalled while the leader still reserves it.

    Once the leader is reaped its id can be reused, so from then on the group
    may only be asked whether it exists. This is the boundary a reused id
    would cross: a ``SIGKILL`` sent after the reap could land on a stranger.
    """
    with spawned(STUBBORN_TREE) as process:
        signal_log.watched.append(process)
        tree.terminate_tree(process)
        delivered = [live for sig, live in signal_log.sent if sig != 0]
        probes = [live for sig, live in signal_log.sent if sig == 0]
        assert delivered == [True, True], "SIGTERM and SIGKILL precede the reap"
        assert probes, "the group must be asked whether it emptied"
        assert not any(probes), "after the reap the group is only probed"


@pytest.mark.usefixtures("short_reap")
def test_a_group_that_cannot_be_confirmed_gone_is_reported(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    """An unconfirmed cleanup replaces the timeout with its own error.

    The caller removes the target directory after a timeout, which is only
    safe once the tree has gone. When that cannot be established the step
    says so, in a form the caller can tell apart from an ordinary timeout.
    """
    monkeypatch.setattr(tree, "group_exited", lambda _group: False)
    with spawned(POLITE_CHILD) as process, pytest.raises(tree.TreeSurvivedError):
        tree.terminate_tree(process)

    monkeypatch.setattr(step, "STEP_TIMEOUT_SECONDS", 0.5)
    program = "import time\ntime.sleep(120)\n"
    with pytest.raises(tree.TreeSurvivedError):
        step.run_step([sys.executable, "-c", program], target_dir=tmp_path)


@pytest.mark.usefixtures("short_reap")
def test_a_step_that_outlives_its_bound_is_killed_and_the_timeout_reraised(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    """The timeout reaches the caller only after the tree has gone."""
    monkeypatch.setattr(step, "STEP_TIMEOUT_SECONDS", 0.5)
    pid_file = tmp_path / "pid"
    program = (
        "import os, signal, time\n"
        "signal.signal(signal.SIGTERM, signal.SIG_IGN)\n"
        f"open({str(pid_file)!r}, 'w').write(str(os.getpid()))\n"
        "time.sleep(120)\n"
    )
    with pytest.raises(subprocess.TimeoutExpired):
        step.run_step([sys.executable, "-c", program], target_dir=tmp_path)
    with pytest.raises(ProcessLookupError):
        os.kill(int(pid_file.read_text(encoding="utf-8")), 0)


@pytest.mark.usefixtures("short_reap")
def test_a_descendant_that_outlives_its_parent_is_killed_too() -> None:
    """The parent exiting on ``SIGTERM`` does not end the wait.

    The step's own process dies politely while its descendant ignores the
    signal. Waiting on the parent alone would return here with the descendant
    still writing into the directory the caller is about to remove.
    """
    with spawned(PARENT_OF_STUBBORN_CHILD) as process:
        tree.terminate_tree(process)
        polite = process.returncode == -signal.SIGTERM
        assert polite, f"the parent should die on SIGTERM, got {process.returncode}"
        assert group_is_empty(process.pid), "the descendant must be gone as well"


@pytest.mark.usefixtures("short_reap")
def test_a_tree_that_ignores_sigterm_throughout_is_killed_as_a_group() -> None:
    """Parent and descendant both outlast ``SIGTERM`` and both get ``SIGKILL``."""
    with spawned(STUBBORN_TREE) as process:
        tree.terminate_tree(process)
        killed = process.returncode == -signal.SIGKILL
        assert killed, f"the parent should need SIGKILL, got {process.returncode}"
        assert group_is_empty(process.pid), "the descendant must be gone as well"


@pytest.mark.usefixtures("short_reap")
def test_a_timed_out_step_has_its_whole_stubborn_tree_killed(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    """The timeout is re-raised only once the descendant has gone too."""
    monkeypatch.setattr(step, "STEP_TIMEOUT_SECONDS", 1.0)
    pid_file = tmp_path / "pid"
    program = (
        "import os\n"
        f"open({str(pid_file)!r}, 'w').write(str(os.getpid()))\n" + STUBBORN_TREE
    )
    with pytest.raises(subprocess.TimeoutExpired):
        step.run_step([sys.executable, "-c", program], target_dir=tmp_path)
    group = int(pid_file.read_text(encoding="utf-8"))
    assert group_is_empty(group), "the whole tree must be gone before the raise"


@pytest.mark.usefixtures("short_reap")
def test_a_descendant_holding_the_pipe_of_an_exited_parent_is_killed(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    """A step whose own process already exited still has its tree removed.

    The parent exits at once and its descendant keeps the captured pipe open,
    so the step never reaches end-of-file and times out with a parent that is
    already dead. That parent is not a reason to skip the group.
    """
    monkeypatch.setattr(step, "STEP_TIMEOUT_SECONDS", 0.5)
    pid_file = tmp_path / "pid"
    program = (
        "import os, subprocess, sys\n"
        f"open({str(pid_file)!r}, 'w').write(str(os.getpid()))\n"
        "subprocess.Popen([sys.executable, '-c', 'import time; time.sleep(120)'])\n"
    )
    with pytest.raises(subprocess.TimeoutExpired):
        step.run_step([sys.executable, "-c", program], target_dir=tmp_path)
    group = int(pid_file.read_text(encoding="utf-8"))
    assert group_is_empty(group), "the descendant must be gone before the raise"
