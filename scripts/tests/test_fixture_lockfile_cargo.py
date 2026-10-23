"""Unit tests for the Cargo invocation layer of the fixture-lockfile gate.

Each operation the gate runs over a fixture is a fixed argv plus the manifest
path, routed through one shared runner so a lockfile that cannot be validated
is reported per fixture rather than aborting the run. These tests pin that
routing and the launch-failure contract; the gate's mode behaviour lives in
``test_check_fixture_lockfiles``.

``seed_and_resolve`` is the one function here that writes, so it is tested at
its own filesystem boundary: a temporary fixture, a real copy, and an injected
resolver stand in for Cargo. The resolver is what the tests observe, so the
seeding can be proven to happen before resolution without a network or a build.
"""

import subprocess  # ruff: ignore[suspicious-subprocess-import] - tests build stand-in CompletedProcess values without running anything.
import typing as typ
from pathlib import Path
from unittest import mock

import pytest
from fixture_lockfile_cargo import (
    cargo_fetch_command,
    cargo_metadata_command,
    cargo_resolve_command,
    fetch_fixture_dependencies,
    refresh_lockfile,
    run_cargo_command,
    run_cargo_metadata,
    seed_and_resolve,
)
from fixture_lockfile_reporting import FixtureLockfileError

if typ.TYPE_CHECKING:
    import collections.abc as cabc

REPO_ROOT = Path(__file__).resolve().parents[2]


def test_run_cargo_command_reports_a_missing_cargo_executable() -> None:
    """A Cargo that cannot start raises the gate's own error, named per fixture."""
    manifest = REPO_ROOT / "crates/rstest-bdd/tests/ui_lints/Cargo.toml"
    oserror = OSError(2, "No such file or directory")
    with (
        mock.patch(
            "fixture_lockfile_cargo.subprocess.run", side_effect=oserror
        ) as spawn,
        pytest.raises(FixtureLockfileError, match="cannot run cargo") as excinfo,
    ):
        run_cargo_command(cargo_metadata_command(manifest), manifest)
    assert spawn.call_args.kwargs["capture_output"] is True, (
        "the runner must capture Cargo output so failures can name it"
    )
    assert spawn.call_args.kwargs["text"] is True, (
        "Cargo output must be decoded as text"
    )
    assert spawn.call_args.kwargs["check"] is False, (
        "a non-zero exit must return the result so every fixture is reported"
    )
    assert isinstance(excinfo.value.__cause__, OSError), (
        "the original launch failure must stay chained for debugging"
    )


def test_thin_wrappers_delegate_to_the_shared_runner() -> None:
    """Every public entry point builds its argv and hands it to one runner."""
    manifest = REPO_ROOT / "crates/rstest-bdd/tests/ui_lints/Cargo.toml"
    with mock.patch("fixture_lockfile_cargo.run_cargo_command") as runner:
        run_cargo_metadata(manifest)
        refresh_lockfile(manifest)
        fetch_fixture_dependencies(manifest)
    assert runner.call_count == 3, (
        "each wrapper must route through the shared Cargo runner exactly once"
    )
    metadata_argv, metadata_manifest = runner.call_args_list[0].args
    refresh_argv, refresh_manifest = runner.call_args_list[1].args
    fetch_argv, fetch_manifest = runner.call_args_list[2].args
    assert metadata_argv == cargo_metadata_command(manifest), (
        "validation must always use the locked metadata argv"
    )
    assert metadata_manifest == manifest, "validation must name the manifest"
    assert refresh_argv == cargo_resolve_command(manifest), (
        "the refresh must re-resolve through cargo metadata, not "
        "generate-lockfile: only the former keeps the versions the existing "
        "lockfile pins"
    )
    assert refresh_manifest == manifest, "refresh must name the manifest it regenerated"
    assert fetch_argv == cargo_fetch_command(manifest), (
        "the prefetch must use the locked fetch argv"
    )
    assert fetch_manifest == manifest, "the prefetch must name the manifest it warmed"


def _stub_resolver(
    observed: list[tuple[str, str]],
) -> cabc.Callable[[Path], subprocess.CompletedProcess[str]]:
    """Return a resolver that records the fixture lockfile it was handed.

    The lockfile's contents are captured by value, because the resolver reads
    the seeded file while it is still in place.

    Returns
    -------
    cabc.Callable[[Path], subprocess.CompletedProcess[str]]
        A resolver appending ``(manifest, lockfile contents)`` to *observed*.
    """

    def resolve(manifest: Path) -> subprocess.CompletedProcess[str]:
        """Record *manifest* and the lockfile contents beside it."""
        lock = manifest.parent / "Cargo.lock"
        observed.append((str(manifest), lock.read_text(encoding="utf-8")))
        return subprocess.CompletedProcess(
            args=["cargo"], returncode=0, stdout="", stderr=""
        )

    return resolve


def test_seed_and_resolve_copies_the_root_lock_before_resolving(
    tmp_path: Path,
) -> None:
    """The workspace lock lands beside the fixture before the resolver runs.

    The copy is the whole point of the function: it pins every shared crate to
    the workspace's choice, leaving Cargo only the fixture's own additions. A
    resolver that saw anything else would resolve to versions the workspace
    did not pick, which is the drift the alignment gate then reports.
    """
    root_lock = tmp_path / "Cargo.lock"
    root_lock.write_text("# workspace resolution\n", encoding="utf-8")
    fixture = tmp_path / "fixtures" / "drifted"
    fixture.mkdir(parents=True)
    manifest = fixture / "Cargo.toml"
    manifest.write_text('[package]\nname = "drifted"\n[workspace]\n', encoding="utf-8")
    (fixture / "Cargo.lock").write_text(
        "# fixture's own resolution\n", encoding="utf-8"
    )
    observed: list[tuple[str, str]] = []

    seed_and_resolve(root_lock, manifest, _stub_resolver(observed))

    assert observed == [(str(manifest), "# workspace resolution\n")], (
        "the resolver must be handed the manifest, with the workspace lock "
        "already copied beside it"
    )
    assert (fixture / "Cargo.lock").read_text(encoding="utf-8") == (
        "# workspace resolution\n"
    ), "the seeded lockfile must persist beside the fixture"


def test_seed_and_resolve_returns_the_resolver_result_unchanged(
    tmp_path: Path,
) -> None:
    """A failing resolution is returned, not raised or rewritten.

    The gate reports each fixture's result itself, so the seeding must stay
    transparent to the failure: converting it here would hide Cargo's own
    diagnostic from the report.
    """
    root_lock = tmp_path / "Cargo.lock"
    root_lock.write_text("# workspace resolution\n", encoding="utf-8")
    manifest = tmp_path / "fixtures" / "broken" / "Cargo.toml"
    manifest.parent.mkdir(parents=True)
    manifest.write_text('[package]\nname = "broken"\n[workspace]\n', encoding="utf-8")
    failure = subprocess.CompletedProcess(
        args=cargo_resolve_command(manifest),
        returncode=101,
        stdout="partial resolution output",
        stderr="error: failed to resolve",
    )

    def failing_resolver(_manifest: Path) -> subprocess.CompletedProcess[str]:
        """Return the recorded failure for whichever manifest it is handed."""
        return failure

    result = seed_and_resolve(root_lock, manifest, failing_resolver)

    assert result is failure, "the resolver's own result must come back unchanged"


def test_seed_and_resolve_restores_a_failed_resolutions_lockfile(
    tmp_path: Path,
) -> None:
    """A resolution that fails leaves the fixture's committed lockfile intact.

    The seed is a starting point, not a resolution: if a failed refresh left
    the workspace lockfile beside the fixture, the next locked validation would
    compare the fixture's *manifest* against the *workspace's* resolution and
    pass, so the gate would report a clean fixture whose lockfile no longer
    describes it. Restoring the committed bytes is what keeps the refresh's
    failure visible to the next check.
    """
    root_lock = tmp_path / "Cargo.lock"
    root_lock.write_text("# workspace resolution\n", encoding="utf-8")
    fixture = tmp_path / "fixtures" / "unrefreshable"
    fixture.mkdir(parents=True)
    manifest = fixture / "Cargo.toml"
    manifest.write_text(
        '[package]\nname = "unrefreshable"\n[workspace]\n', encoding="utf-8"
    )
    committed = "# the fixture's own resolution\n"
    (fixture / "Cargo.lock").write_text(committed, encoding="utf-8")

    def failing_resolver(_manifest: Path) -> subprocess.CompletedProcess[str]:
        """Return a failed resolution for whichever manifest it is handed."""
        return subprocess.CompletedProcess(
            args=cargo_resolve_command(manifest),
            returncode=101,
            stdout="partial resolution output",
            stderr="error: failed to resolve",
        )

    seed_and_resolve(root_lock, manifest, failing_resolver)

    assert (fixture / "Cargo.lock").read_text(encoding="utf-8") == committed, (
        "a failed resolution must leave the committed lockfile as it was found"
    )


def test_seed_and_resolve_restores_the_lockfile_when_the_resolver_raises(
    tmp_path: Path,
) -> None:
    """An interrupted refresh restores the lockfile and re-raises the interrupt.

    A refresh is long enough for a cancellation to land inside it, and
    ``KeyboardInterrupt`` is what a cancelled run raises. The restoration must
    therefore sit in a ``finally`` rather than on the failure result alone:
    without it, an abandoned refresh would strand the workspace lockfile beside
    the fixture, which is the state the next check reads as healthy.
    """
    root_lock = tmp_path / "Cargo.lock"
    root_lock.write_text("# workspace resolution\n", encoding="utf-8")
    fixture = tmp_path / "fixtures" / "cancelled"
    fixture.mkdir(parents=True)
    manifest = fixture / "Cargo.toml"
    manifest.write_text(
        '[package]\nname = "cancelled"\n[workspace]\n', encoding="utf-8"
    )
    committed = "# the fixture's own resolution\n"
    (fixture / "Cargo.lock").write_text(committed, encoding="utf-8")

    def interrupted_resolver(_manifest: Path) -> subprocess.CompletedProcess[str]:
        """Interrupt the refresh part-way through, as a cancelled run does."""
        raise KeyboardInterrupt

    with pytest.raises(KeyboardInterrupt):
        seed_and_resolve(root_lock, manifest, interrupted_resolver)

    assert (fixture / "Cargo.lock").read_text(encoding="utf-8") == committed, (
        "an interrupted refresh must leave the committed lockfile as it was found"
    )


def test_seed_and_resolve_restores_the_lockfile_when_the_seed_copy_fails(
    tmp_path: Path,
) -> None:
    """A failed seed copy restores the lockfile it had already truncated.

    ``shutil.copyfile`` opens the destination before it copies into it, so the
    fixture's committed ``Cargo.lock`` is already truncated when the copy can
    still fail -- on a cancelled run, or a full disk. Restoring only from the
    resolver's failure would leave that file behind, destroying a resolution
    the fixture did commit, and the next run would report Cargo's ``--locked``
    refusal against a lockfile this gate wrote rather than the fixture's own.
    """
    root_lock = tmp_path / "Cargo.lock"
    root_lock.write_text("# workspace resolution\n", encoding="utf-8")
    fixture = tmp_path / "fixtures" / "truncated"
    fixture.mkdir(parents=True)
    manifest = fixture / "Cargo.toml"
    manifest.write_text(
        '[package]\nname = "truncated"\n[workspace]\n', encoding="utf-8"
    )
    committed = "# the fixture's own resolution\n"
    (fixture / "Cargo.lock").write_text(committed, encoding="utf-8")

    def failing_copy(_src: Path, dst: Path) -> None:
        """Truncate the destination, then fail, exactly as a cancelled copy does."""
        Path(dst).write_bytes(b"")
        raise KeyboardInterrupt

    with (
        mock.patch("fixture_lockfile_cargo.shutil.copyfile", failing_copy),
        pytest.raises(KeyboardInterrupt),
    ):
        seed_and_resolve(root_lock, manifest, _stub_resolver([]))

    assert (fixture / "Cargo.lock").read_text(encoding="utf-8") == committed, (
        "a seed copy that fails part-way must leave the committed lockfile as it "
        "was found, not truncated"
    )


def test_seed_and_resolve_removes_a_seed_the_fixture_never_committed(
    tmp_path: Path,
) -> None:
    """A fixture with no committed lockfile is left with none after a failure.

    Removing the seeded file rather than emptying it matters because Cargo
    treats an absent lockfile and a truncated one differently: a truncated one
    parses as a lockfile with no packages, which is a resolution the fixture
    never committed.
    """
    root_lock = tmp_path / "Cargo.lock"
    root_lock.write_text("# workspace resolution\n", encoding="utf-8")
    fixture = tmp_path / "fixtures" / "uncommitted"
    fixture.mkdir(parents=True)
    manifest = fixture / "Cargo.toml"
    manifest.write_text(
        '[package]\nname = "uncommitted"\n[workspace]\n', encoding="utf-8"
    )

    def failing_resolver(_manifest: Path) -> subprocess.CompletedProcess[str]:
        """Return a failed resolution for whichever manifest it is handed."""
        return subprocess.CompletedProcess(
            args=cargo_resolve_command(manifest),
            returncode=101,
            stdout="",
            stderr="error: failed to resolve",
        )

    seed_and_resolve(root_lock, manifest, failing_resolver)

    assert not (fixture / "Cargo.lock").exists(), (
        "a fixture that committed no lockfile must not be given one by a failed refresh"
    )
