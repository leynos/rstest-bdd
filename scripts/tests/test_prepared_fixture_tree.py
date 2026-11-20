"""Unit tests for the prepared-fixture step's process-tree termination.

A step that outlives its bound has to be gone, with every descendant, before
the driver removes the target directory it was writing to. These cases drive
``terminate_tree`` and the timeout path of ``run_step`` against small child
processes: a tree that honours ``SIGTERM``, one that ignores it, a descendant
that outlives its parent, and a descendant that holds the captured pipe of a
parent that already exited. They signal process groups, so they are skipped
where the platform has none.
"""

import contextlib
import os
import signal
import subprocess  # ruff: ignore[suspicious-subprocess-import] - spawns a fixed interpreter argv; no shell.
import sys
import typing as typ
from pathlib import Path

import pytest

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

import prepared_fixture_step as step

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

#: These cases signal process groups, which only POSIX has.
posix_only = pytest.mark.skipif(os.name != "posix", reason="needs process groups")


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
    monkeypatch.setattr(step, "TREE_REAP_SECONDS", 0.5)


@posix_only
@pytest.mark.usefixtures("short_reap")
def test_a_tree_that_ignores_sigterm_is_killed_within_the_grace_period() -> None:
    """``SIGKILL`` follows when the polite signal is ignored."""
    with spawned(STUBBORN_CHILD) as process:
        step.terminate_tree(process)
        killed = process.returncode == -signal.SIGKILL
        assert killed, f"expected SIGKILL, got status {process.returncode}"


@posix_only
@pytest.mark.usefixtures("short_reap")
def test_a_tree_that_honours_sigterm_is_not_escalated() -> None:
    """A child that exits on ``SIGTERM`` is never sent ``SIGKILL``."""
    with spawned(POLITE_CHILD) as process:
        step.terminate_tree(process)
        polite = process.returncode == -signal.SIGTERM
        assert polite, f"expected SIGTERM, got status {process.returncode}"


@posix_only
def test_a_finished_process_is_left_alone() -> None:
    """A process that already exited is not signalled.

    Its process group id may have been reused, so signalling it could reach an
    unrelated tree.
    """
    with subprocess.Popen(
        [sys.executable, "-c", "pass"], start_new_session=True, text=True
    ) as process:
        process.wait()
        step.terminate_tree(process)
        assert process.returncode == 0, "a finished process keeps its status"


@posix_only
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


@posix_only
@pytest.mark.usefixtures("short_reap")
def test_a_descendant_that_outlives_its_parent_is_killed_too() -> None:
    """The parent exiting on ``SIGTERM`` does not end the wait.

    The step's own process dies politely while its descendant ignores the
    signal. Waiting on the parent alone would return here with the descendant
    still writing into the directory the caller is about to remove.
    """
    with spawned(PARENT_OF_STUBBORN_CHILD) as process:
        step.terminate_tree(process)
        polite = process.returncode == -signal.SIGTERM
        assert polite, f"the parent should die on SIGTERM, got {process.returncode}"
        assert group_is_empty(process.pid), "the descendant must be gone as well"


@posix_only
@pytest.mark.usefixtures("short_reap")
def test_a_tree_that_ignores_sigterm_throughout_is_killed_as_a_group() -> None:
    """Parent and descendant both outlast ``SIGTERM`` and both get ``SIGKILL``."""
    with spawned(STUBBORN_TREE) as process:
        step.terminate_tree(process)
        killed = process.returncode == -signal.SIGKILL
        assert killed, f"the parent should need SIGKILL, got {process.returncode}"
        assert group_is_empty(process.pid), "the descendant must be gone as well"


@posix_only
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


@posix_only
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
