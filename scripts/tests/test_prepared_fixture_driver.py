"""Unit tests for the prepared-fixture workflow's driver.

``test_check_prepared_fixture_workflow`` covers the judgements: what each
outcome must mean. This file covers the code around them, which otherwise runs
only in a cold end-to-end job: the argv each step is given, the fixture edit
and its restore, the order of the steps, and the exit status the driver
reports. None of the cases builds or spawns anything; each replaces the step
runner. The process-tree termination behind a step's timeout is covered in
``test_prepared_fixture_tree``.
"""

import os
import shutil
import subprocess  # ruff: ignore[suspicious-subprocess-import] - only constructs values and exceptions; spawns nothing.
import sys
import typing as typ
from pathlib import Path

import pytest

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

import check_prepared_fixture_workflow as workflow
import prepared_fixture_step as step
from prepared_fixture_judgements import StepFailureError
from test_prepare_nested_fixtures_makefile_contract import RUNNER, TARGET, recipe

if typ.TYPE_CHECKING:
    import collections.abc as cabc

Result = subprocess.CompletedProcess[str]


def a_result(returncode: int = 0, stderr: str = "") -> Result:
    """Return a completed step carrying *returncode* and *stderr*."""
    return subprocess.CompletedProcess(["step"], returncode, "", stderr)


def a_refusal() -> Result:
    """Return the outcome of a timed run the stamp refused."""
    return a_result(returncode=100, stderr=f"run `{step.REMEDY}` first")


def test_the_timed_step_names_the_runner_target_and_the_exact_filter() -> None:
    """The timed argv is the Makefile's run half with the one timed test.

    The filter is spelled out rather than read from the module: a filter that
    drifted to match nothing makes nextest exit non-zero, which step one would
    otherwise be tempted to read as a refusal.
    """
    expected = [
        "make",
        RUNNER,
        "EXPR=binary_id(cargo-bdd::cli) & test(=list_steps_runs)",
    ]
    assert step.timed_test_argv() == expected, "the timed argv drifted"
    assert recipe(RUNNER), "the run half must exist in the Makefile"


def test_the_preparation_step_is_the_documented_remedy() -> None:
    """The remedy the driver runs is the one the refusal tells a reader to run."""
    assert step.preparation_argv() == ["make", TARGET], "the remedy argv drifted"
    spelled = " ".join(step.preparation_argv())
    assert spelled == step.REMEDY, "the driver must run the remedy it names"
    assert recipe(TARGET), "the remedy must exist in the Makefile"


@pytest.fixture
def fixture_source(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> Path:
    """Point the workflow at a temporary checkout holding one fixture source.

    Returns
    -------
    Path
        The fixture file the negative control edits.
    """
    monkeypatch.setattr(workflow, "REPO_ROOT", tmp_path)
    source = tmp_path / workflow.FIXTURE_SOURCE
    source.parent.mkdir(parents=True)
    source.write_bytes(b"pub fn fixture() {}\n")
    return source


def a_runner(
    outcomes: list[Result | Exception], seen: list[bytes], source: Path
) -> cabc.Callable[..., Result]:
    """Return a step runner that replays *outcomes* and records the fixture.

    Each call appends the fixture's bytes at that moment to *seen*, so a case
    can assert on what a step would have built from.

    Returns
    -------
    cabc.Callable[..., Result]
        A stand-in for ``run_step``.
    """
    pending = iter(outcomes)

    def run(argv: list[str], *, target_dir: Path) -> Result:
        """Record the fixture's bytes, then replay the next outcome."""
        del argv, target_dir
        seen.append(source.read_bytes())
        outcome = next(pending)
        if isinstance(outcome, Exception):
            raise outcome
        return outcome

    return run


@pytest.mark.parametrize(
    ("outcome", "raised"),
    [
        pytest.param(a_refusal(), None, id="the control held"),
        pytest.param(a_result(returncode=0), StepFailureError, id="judgement failed"),
        pytest.param(OSError("spawn failed"), OSError, id="the runner raised"),
    ],
)
def test_the_negative_control_restores_the_fixture_whatever_happens(
    fixture_source: Path,
    monkeypatch: pytest.MonkeyPatch,
    outcome: Result | Exception,
    raised: type[Exception] | None,
) -> None:
    """The fixture is edited for the run and restored on every way out."""
    original = fixture_source.read_bytes()
    seen: list[bytes] = []
    monkeypatch.setattr(workflow, "run_step", a_runner([outcome], seen, fixture_source))

    if raised is None:
        workflow.expect_mutation_invalidates_the_stamp(fixture_source.parent)
    else:
        with pytest.raises(raised):
            workflow.expect_mutation_invalidates_the_stamp(fixture_source.parent)

    edited = original + workflow.NEGATIVE_CONTROL_EDIT
    assert seen == [edited], "the step must run against the edited fixture"
    restored = fixture_source.read_bytes() == original
    assert restored, "the fixture must be restored to its original bytes"


def test_the_workflow_runs_its_four_steps_in_order_and_restores_the_fixture(
    fixture_source: Path, tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    """Refuse, prepare, pass without rebuilding, then refuse the edited tree."""
    original = fixture_source.read_bytes()
    target_dir = tmp_path / "target"
    target_dir.mkdir()
    argvs: list[list[str]] = []

    def run(argv: list[str], *, target_dir: Path) -> Result:
        """Answer each step as a healthy workflow would, in call order."""
        argvs.append(argv)
        if len(argvs) == 2:
            return a_result(returncode=0)
        if len(argvs) == 3:
            step.build_report_path(target_dir).write_text(
                f"{step.BUILD_MARKER} source={step.GATE_SOURCE} compiling=0\n"
                f"{step.BUILD_MARKER} source={step.TIMED_SOURCE} compiling=0\n",
                encoding="utf-8",
            )
            return a_result(returncode=0)
        return a_refusal()

    monkeypatch.setattr(workflow, "run_step", run)
    workflow.run_workflow(target_dir)

    timed, remedy = step.timed_test_argv(), step.preparation_argv()
    assert argvs == [timed, remedy, timed, timed], "the steps ran out of order"
    restored = fixture_source.read_bytes() == original
    assert restored, "the fixture must be restored to its original bytes"


def test_a_failed_step_stops_the_workflow_before_the_fixture_is_edited(
    fixture_source: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    """A first step that passes is a failure, and nothing later runs."""
    original = fixture_source.read_bytes()
    seen: list[bytes] = []
    monkeypatch.setattr(
        workflow, "run_step", a_runner([a_result(returncode=0)], seen, fixture_source)
    )
    with pytest.raises(StepFailureError):
        workflow.run_workflow(fixture_source.parent)
    assert seen == [original], "no step may run after the first one fails"


@pytest.mark.parametrize(
    ("raised", "expected"),
    [
        pytest.param(None, 0, id="every step held"),
        pytest.param(StepFailureError("step 1 passed"), 1, id="a step failed"),
        pytest.param(subprocess.TimeoutExpired(["make"], 1), 1, id="a step timed out"),
    ],
)
def test_the_exit_status_reports_the_workflows_outcome(
    monkeypatch: pytest.MonkeyPatch,
    raised: Exception | None,
    expected: int,
) -> None:
    """Zero only when the workflow held; the private directory is removed."""
    used: list[Path] = []

    def run_workflow(target_dir: Path) -> None:
        """Stand in for the workflow, raising the case's failure if any."""
        used.append(target_dir)
        assert target_dir.is_dir(), "the workflow is given a real directory"
        if raised is not None:
            raise raised

    monkeypatch.setattr(workflow, "run_workflow", run_workflow)
    assert workflow.main([]) == expected, "the exit status misreports the outcome"
    assert not used[0].exists(), "the private target directory must be removed"


def test_an_unconfirmed_cleanup_fails_and_preserves_the_directory(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """A tree that may still be running keeps the directory it writes to."""
    used: list[Path] = []

    def run_workflow(target_dir: Path) -> None:
        """Stand in for a workflow whose timed-out step left processes behind."""
        used.append(target_dir)
        message = "process group 1 still has a member after SIGKILL"
        raise step.TreeSurvivedError(message)

    monkeypatch.setattr(workflow, "run_workflow", run_workflow)
    try:
        assert workflow.main([]) == 1, "an unconfirmed cleanup is a failure"
        assert used[0].is_dir(), "the directory must survive an unconfirmed cleanup"
    finally:
        for directory in used:
            directory.rmdir()


@pytest.mark.skipif(os.name != "posix", reason="needs process groups")
def test_a_leader_that_cannot_be_reaped_still_reaches_the_driver(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """A process that survives ``SIGKILL`` fails the run without hanging it.

    Every bounded wait for the step's process is made to time out, which is
    what a process stuck in the kernel looks like. The step must then give
    up on the process, not wait for it without a bound, so the error reaches
    the driver and the driver keeps the directory.
    """
    monkeypatch.setattr(step, "STEP_TIMEOUT_SECONDS", 0.5)
    monkeypatch.setattr(step, "TREE_REAP_SECONDS", 0.5)
    real_wait = subprocess.Popen.wait
    leaders: list[subprocess.Popen[str]] = []
    unbounded: list[int] = []
    used: list[Path] = []

    def wait(self: subprocess.Popen[str], timeout: float | None = None) -> int:
        """Time out every bounded wait, and record any unbounded one."""
        leaders.append(self)
        if timeout is None:
            unbounded.append(self.pid)
            return real_wait(self)
        raise subprocess.TimeoutExpired(self.args, timeout)

    def run_workflow(target_dir: Path) -> None:
        """Run one step that outlives its bound."""
        used.append(target_dir)
        program = "import time\ntime.sleep(120)\n"
        step.run_step([sys.executable, "-c", program], target_dir=target_dir)

    monkeypatch.setattr(subprocess.Popen, "wait", wait)
    monkeypatch.setattr(workflow, "run_workflow", run_workflow)
    try:
        assert workflow.main([]) == 1, "an unreapable leader is a failure"
        assert used[0].is_dir(), "the directory must survive an unconfirmed cleanup"
        assert not unbounded, "the step must not wait for the leader unbounded"
    finally:
        # The process was killed; only the wait was made to fail. Reap it and
        # remove the directory the driver rightly kept.
        for leader in leaders:
            real_wait(leader)
        for directory in used:
            shutil.rmtree(directory, ignore_errors=True)


def test_keep_leaves_the_private_directory_for_inspection(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """``--keep`` is the one way the directory outlives the run."""
    used: list[Path] = []
    monkeypatch.setattr(workflow, "run_workflow", used.append)
    try:
        assert workflow.main(["--keep"]) == 0, "a held workflow exits zero"
        assert used[0].is_dir(), "`--keep` must leave the directory in place"
    finally:
        for directory in used:
            directory.rmdir()
