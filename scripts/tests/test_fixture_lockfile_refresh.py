"""Unit tests for the refresh path and the gate's check-path wiring.

A refresh is the one gate operation that rewrites committed artefacts, so it is
tested at its seam rather than end to end: which fixtures are seeded, and what
the seeding is handed. The check path needs its own tests for the same reason
from the other side — validation and the alignment comparison are two halves of
one exit code, and a gate that reported only the first half would accept a
fixture the comparison rejects.

The check path is therefore also exercised once at its own boundary, over a
temporary root: stubbed Cargo validation joined to the gate's real comparison,
so the exit code under test is the one a drifted fixture genuinely produces.
"""

import subprocess  # ruff: ignore[suspicious-subprocess-import] - tests build stand-in CompletedProcess values without running anything.
import typing as typ
from pathlib import Path
from unittest import mock

import pytest
from check_fixture_lockfiles import (
    check_fixtures,
    refresh_fixtures,
    seed_fixture_lockfile,
)
from fixture_lockfile_alignment import is_seed_excluded
from fixture_lockfile_cargo import cargo_metadata_command, cargo_resolve_command
from fixture_lockfile_discovery import discover_fixture_manifests

if typ.TYPE_CHECKING:
    import collections.abc as cabc

REPO_ROOT = Path(__file__).resolve().parents[2]

#: Fixtures that exercise the two seeding branches. ``ui_lints`` is exempt from
#: the alignment requirement but still seeded, which is what makes it a useful
#: stand-in for the ordinary branch.
SEEDED_MANIFEST = REPO_ROOT / "crates/rstest-bdd/tests/ui_lints/Cargo.toml"
SEED_EXCLUDED_MANIFEST = REPO_ROOT / "tests/fixtures/published-gpui-0-2-2/Cargo.toml"

#: A second seeded fixture, so a test can prove the loop carries on past one
#: fixture's failure rather than stopping there.
LATER_MANIFEST = (
    REPO_ROOT / "crates/rstest-bdd/tests/fixtures/rebuild_invalidation/Cargo.toml"
)


@pytest.mark.parametrize(
    ("manifest", "is_seeded"),
    [
        (SEEDED_MANIFEST, True),
        (SEED_EXCLUDED_MANIFEST, False),
    ],
)
def test_seed_fixture_lockfile_routes_by_the_seed_exclusion(
    manifest: Path, *, is_seeded: bool
) -> None:
    """A seeded fixture is copied from the root lock; an excluded one is not.

    The two branches exist because seeding is right for every fixture except
    those written to resolve outside the workspace, so the routing is the
    behaviour worth pinning: the exclusion set decides it, and a fixture in
    neither set must take the seeding branch.
    """
    assert is_seed_excluded(manifest, REPO_ROOT) != is_seeded, (
        "this test leans on the fixture's own classification, which must hold"
    )
    with (
        mock.patch("check_fixture_lockfiles.seed_and_resolve") as seeded,
        mock.patch("check_fixture_lockfiles.refresh_lockfile") as resolve,
    ):
        seed_fixture_lockfile(REPO_ROOT, manifest)

    if is_seeded:
        assert seeded.call_args.args == (
            REPO_ROOT / "Cargo.lock",
            manifest,
            resolve,
        ), "a seeded fixture must be copied from the root lock before resolving"
        assert not resolve.called, (
            "the seeded branch resolves through seed_and_resolve's own call, "
            "rather than invoking the resolver here as well"
        )
    else:
        assert resolve.call_args.args == (manifest,), (
            "a seed-excluded fixture must be re-resolved from its own lockfile"
        )
        assert not seeded.called, (
            "seeding would replace the very resolution the fixture proves"
        )


def _cargo_stand_in(
    argv: list[str], *, returncode: int = 0, stderr: str = ""
) -> subprocess.CompletedProcess[str]:
    """Return a stand-in for a ``cargo`` invocation that ran *argv*.

    Every stand-in in this module is that same value with a different argv, so
    the shape is built in one place rather than restated in each of them.

    Returns
    -------
    subprocess.CompletedProcess[str]
        The stand-in, carrying *argv* as the command it appears to have run.
    """
    return subprocess.CompletedProcess(
        args=argv, returncode=returncode, stdout="", stderr=stderr
    )


def _resolve_result(
    manifest: Path, *, returncode: int = 0, stderr: str = ""
) -> subprocess.CompletedProcess[str]:
    """Return a stand-in for a resolving ``cargo metadata`` over *manifest*."""
    return _cargo_stand_in(
        cargo_resolve_command(manifest), returncode=returncode, stderr=stderr
    )


def test_a_failed_preparation_fails_that_fixtures_validation() -> None:
    """A refresh that could not resolve fails the fixture it was asked to refresh.

    The preparation restores the fixture's committed lockfile when it fails, so
    the validating ``cargo metadata`` that followed would resolve that restored
    lockfile and pass. Reporting validation's pass would answer a question the
    run never asked: the fixture is refreshable only if the refresh resolved, so
    the preparation's own failure is what the fixture must be charged with.
    """
    unresolved = _resolve_result(
        SEEDED_MANIFEST, returncode=101, stderr="error: failed to resolve"
    )
    with (
        mock.patch("check_fixture_lockfiles.seed_and_resolve", return_value=unresolved),
        mock.patch("check_fixture_lockfiles.run_cargo_metadata") as validate,
        mock.patch("check_fixture_lockfiles.print_failures") as emit_failures,
    ):
        exit_code = refresh_fixtures(REPO_ROOT, [SEEDED_MANIFEST])

    assert exit_code == 1, "a failed preparation must fail the refresh gate"
    assert not validate.called, (
        "a fixture whose preparation failed must not be validated: its restored "
        "lockfile would resolve and report a pass the refresh never earned"
    )
    report = emit_failures.call_args.args[0][0]
    assert "ui_lints" in report, "the report must name the fixture that failed"
    assert "error: failed to resolve" in report, (
        "the report must carry the preparation's own Cargo stderr"
    )


def test_a_failed_preparation_does_not_stop_the_remaining_fixtures() -> None:
    """The loop carries on past a failed preparation to validate the rest.

    One gate run reports every fixture, so a refresh that fails for the first
    fixture must not hide a stale lockfile behind it. Only the first fixture's
    preparation fails here, so the second one's validation is the evidence that
    the run kept going rather than abandoning the set at the failure.
    """

    def prepare(
        root_lock: Path, manifest: Path, resolve: object
    ) -> subprocess.CompletedProcess[str]:
        """Fail the preparation for the first fixture only."""
        returncode = 101 if manifest == SEEDED_MANIFEST else 0
        return _resolve_result(manifest, returncode=returncode, stderr="nope")

    with (
        mock.patch("check_fixture_lockfiles.seed_and_resolve", side_effect=prepare),
        mock.patch(
            "check_fixture_lockfiles.run_cargo_metadata",
            side_effect=_metadata_result,
        ) as validate,
        mock.patch("check_fixture_lockfiles.print_failures"),
    ):
        exit_code = refresh_fixtures(REPO_ROOT, [SEEDED_MANIFEST, LATER_MANIFEST])

    assert exit_code == 1, "the failed preparation must still fail the gate"
    assert [call.args[0] for call in validate.call_args_list] == [LATER_MANIFEST], (
        "every fixture after the failed preparation must still be validated"
    )


def test_a_failed_preparation_report_quotes_the_resolving_command() -> None:
    """The failure report quotes the argv that failed, not the one that follows.

    A preparation's argv is the resolving ``cargo metadata``, which is the
    command a reader must rerun to see the failure. The mode's validating argv
    is the locked one, and it succeeds once the committed lockfile is restored,
    so quoting it would hand the reader a command that contradicts the report.
    """
    unresolved = _resolve_result(
        SEEDED_MANIFEST, returncode=101, stderr="error: failed to resolve"
    )
    with (
        mock.patch("check_fixture_lockfiles.seed_and_resolve", return_value=unresolved),
        mock.patch("check_fixture_lockfiles.run_cargo_metadata"),
        mock.patch("check_fixture_lockfiles.print_failures") as emit_failures,
    ):
        refresh_fixtures(REPO_ROOT, [SEEDED_MANIFEST])

    report = emit_failures.call_args.args[0][0]
    assert f"command: {' '.join(cargo_resolve_command(SEEDED_MANIFEST))}" in report, (
        "the report must quote the resolving command, which is the one that "
        "actually failed"
    )
    validating = "command: " + " ".join(cargo_metadata_command(SEEDED_MANIFEST))
    assert validating not in report, (
        "the report must not quote the validating command, which succeeds over "
        "the restored lockfile"
    )


def _metadata_result(
    manifest: Path, *, returncode: int = 0, stderr: str = ""
) -> subprocess.CompletedProcess[str]:
    """Return a stand-in for a locked ``cargo metadata`` over *manifest*."""
    return _cargo_stand_in(
        cargo_metadata_command(manifest), returncode=returncode, stderr=stderr
    )


@pytest.mark.parametrize(("alignment", "expected"), [(0, 0), (1, 1)])
def test_check_fixtures_propagates_an_alignment_failure(
    alignment: int, expected: int
) -> None:
    """Validation and alignment are separate halves of one exit code.

    The alignment pass reads the lockfiles only once every one of them resolved,
    so a fixture whose graph drifted past the workspace must still fail the gate
    even though ``cargo metadata --locked`` accepted its lockfile.
    """
    manifests = [SEEDED_MANIFEST]
    with (
        mock.patch(
            "check_fixture_lockfiles.run_cargo_metadata",
            side_effect=_metadata_result,
        ),
        mock.patch(
            "check_fixture_lockfiles.check_alignment_or_failure",
            return_value=alignment,
        ) as compare,
        mock.patch("check_fixture_lockfiles.print_failures"),
        mock.patch("check_fixture_lockfiles.print_check_summary"),
    ):
        exit_code = check_fixtures(REPO_ROOT, manifests)

    compare.assert_called_once_with(REPO_ROOT)
    assert exit_code == expected, (
        "the gate's exit code must be the alignment verdict once every "
        "lockfile has resolved"
    )


def test_check_fixtures_skips_alignment_when_a_lockfile_is_stale() -> None:
    """A stale lockfile fails the gate before the comparison is reached.

    The halves are joined by a short-circuit, so the comparison never reads a
    lockfile that has already failed validation; running it regardless would
    report drift computed from a lockfile known to be stale.
    """
    stale = _metadata_result(SEEDED_MANIFEST, returncode=101, stderr="stale lockfile")
    with (
        mock.patch("check_fixture_lockfiles.run_cargo_metadata", return_value=stale),
        mock.patch("check_fixture_lockfiles.check_alignment_or_failure") as compare,
        mock.patch("check_fixture_lockfiles.print_failures"),
        mock.patch("check_fixture_lockfiles.print_check_summary"),
    ):
        exit_code = check_fixtures(REPO_ROOT, [SEEDED_MANIFEST])

    assert exit_code == 1, "a stale lockfile must fail the gate"
    assert not compare.called, (
        "a stale lockfile must fail the gate before the alignment pass runs"
    )


CRATES_IO = "registry+https://github.com/rust-lang/crates.io-index"

#: The workspace resolution the joined test below is compared against.
ROOT_LOCK = (
    f'[[package]]\nname = "serde"\nversion = "1.0.228"\nsource = "{CRATES_IO}"\n'
)

#: A fixture graph that adds only its own crate to the workspace's, which is the
#: one addition the comparison accepts.
ALIGNED_FIXTURE_LOCK = (
    '[[package]]\nname = "probe-fixture"\nversion = "0.1.0"\n'
    f'[[package]]\nname = "serde"\nversion = "1.0.228"\nsource = "{CRATES_IO}"\n'
)

#: A fixture graph resolved away from the workspace's, so the nested build would
#: compile this ``dashmap`` a second time.
DRIFTED_FIXTURE_LOCK = (
    '[[package]]\nname = "probe-fixture"\nversion = "0.1.0"\n'
    f'[[package]]\nname = "dashmap"\nversion = "5.5.3"\nsource = "{CRATES_IO}"\n'
)


def test_check_fixtures_joins_cargo_validation_to_the_alignment_comparison(
    tmp_path: Path,
    capsys: pytest.CaptureFixture[str],
    write_manifest: cabc.Callable[[Path, str], Path],
) -> None:
    """A temporary repository's exit code follows its fixture's resolved graph.

    Both halves of the check run here as the gate runs them: discovery finds the
    fixture, the stubbed ``cargo metadata`` accepts its lockfile — a stand-in,
    because a real validation resolves against the network — and the comparison
    reads the temporary lockfiles for real. Asserting the aligned and the
    drifted graph in one test is what rules out a gate that answers with the
    stub's verdict for either: only a comparison joined to the validated fixture
    can tell the two apart.
    """
    manifest = write_manifest(tmp_path, "probe")
    (tmp_path / "Cargo.lock").write_text(ROOT_LOCK, encoding="utf-8")
    lockfile = manifest.parent / "Cargo.lock"
    lockfile.write_text(ALIGNED_FIXTURE_LOCK, encoding="utf-8")

    manifests = discover_fixture_manifests(tmp_path)
    assert manifests == [manifest], (
        "the temporary fixture must be discovered, so the gate under test "
        "visits the graph this test wrote"
    )

    with mock.patch(
        "check_fixture_lockfiles.run_cargo_metadata",
        side_effect=_metadata_result,
    ):
        assert check_fixtures(tmp_path, manifests) == 0, (
            "a fixture adding only its own crate must leave the gate passing"
        )
        assert not capsys.readouterr().err, (
            "an aligned fixture must report nothing to standard error"
        )

        lockfile.write_text(DRIFTED_FIXTURE_LOCK, encoding="utf-8")
        assert check_fixtures(tmp_path, manifests) == 1, (
            "a fixture pinned away from the workspace graph must fail the gate"
        )

    reported = capsys.readouterr().err
    assert "dashmap 5.5.3" in reported, (
        "the failure must name the divergent package the nested build would "
        "compile a second time"
    )
    assert f"fixture {manifest.as_posix()} resolves 1 package(s)" in reported, (
        "the failure must open on the fixture whose graph drifted, named the way "
        "the renderer names a manifest"
    )


def test_check_fixtures_reports_an_unreadable_lockfile_and_does_not_raise(
    tmp_path: Path,
    capsys: pytest.CaptureFixture[str],
    write_manifest: cabc.Callable[[Path, str], Path],
) -> None:
    """A lockfile the gate cannot parse fails it; it never reaches the caller.

    The alignment comparison declines to invent a graph for a lockfile it
    cannot read, so it raises — which is right for a comparison and wrong at
    the check path's boundary. Without the boundary the raise escapes
    ``check_fixtures`` as a traceback, and the gate exits on an unhandled
    exception instead of the ``error:`` report and exit code 1 its own module
    docstring promises. This is the regression pin for that: the assertion is
    that the call *returns*, so a re-raised error fails here rather than
    surfacing as a crash in the gate that calls it.
    """
    manifest = write_manifest(tmp_path, "probe")
    lockfile = manifest.parent / "Cargo.lock"
    lockfile.write_text(ALIGNED_FIXTURE_LOCK, encoding="utf-8")
    # A root lockfile truncated mid-header — "[[package]" never closes its
    # array declaration — so it is not valid TOML at all and the read raises on
    # the parse, before any package table is consulted. Truncation is the shape
    # a half-written lockfile has, and the input has to be malformed rather
    # than merely table-less: valid TOML without a package table parses into an
    # empty graph, which the comparison reports as a divergence rather than as
    # an unreadable file. The fixture's own lockfile is left readable so the
    # failure can only come from the root side.
    (tmp_path / "Cargo.lock").write_text("[[package]\n", encoding="utf-8")

    manifests = discover_fixture_manifests(tmp_path)
    with mock.patch(
        "check_fixture_lockfiles.run_cargo_metadata",
        side_effect=_metadata_result,
    ):
        exit_code = check_fixtures(tmp_path, manifests)

    assert exit_code == 1, (
        "an unreadable lockfile must fail the gate through its exit code, not "
        "escape as an unhandled exception"
    )
    reported = capsys.readouterr().err
    assert "error: cannot read lockfile" in reported, (
        "the failure must use the gate's own error report wording"
    )
    assert "as TOML" in reported, (
        "the report must name the condition, so the reader knows what to fix"
    )
