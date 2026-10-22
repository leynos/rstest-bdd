"""Unit tests for the refresh path and the gate's check-path wiring.

A refresh is the one gate operation that rewrites committed artefacts, so it is
tested at its seam rather than end to end: which fixtures are seeded, and what
the seeding is handed. The check path needs its own tests for the same reason
from the other side — validation and the alignment comparison are two halves of
one exit code, and a gate that reported only the first half would accept a
fixture the comparison rejects.
"""

import subprocess  # ruff: ignore[suspicious-subprocess-import] - tests build stand-in CompletedProcess values without running anything.
from pathlib import Path
from unittest import mock

import pytest
from check_fixture_lockfiles import check_fixtures, seed_fixture_lockfile
from fixture_lockfile_alignment import is_seed_excluded

REPO_ROOT = Path(__file__).resolve().parents[2]

#: Fixtures that exercise the two seeding branches. ``ui_lints`` is exempt from
#: the alignment requirement but still seeded, which is what makes it a useful
#: stand-in for the ordinary branch.
SEEDED_MANIFEST = REPO_ROOT / "crates/rstest-bdd/tests/ui_lints/Cargo.toml"
SEED_EXCLUDED_MANIFEST = REPO_ROOT / "tests/fixtures/published-gpui-0-2-2/Cargo.toml"


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


def _metadata_result(
    manifest: Path, *, returncode: int = 0, stderr: str = ""
) -> subprocess.CompletedProcess[str]:
    """Return a stand-in for a locked ``cargo metadata`` over *manifest*."""
    return subprocess.CompletedProcess(
        args=["cargo", "metadata", "--locked", "--manifest-path", str(manifest)],
        returncode=returncode,
        stdout="",
        stderr=stderr,
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
            "check_fixture_lockfiles.check_alignment", return_value=alignment
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
        mock.patch("check_fixture_lockfiles.check_alignment") as compare,
        mock.patch("check_fixture_lockfiles.print_failures"),
        mock.patch("check_fixture_lockfiles.print_check_summary"),
    ):
        exit_code = check_fixtures(REPO_ROOT, [SEEDED_MANIFEST])

    assert exit_code == 1, "a stale lockfile must fail the gate"
    assert not compare.called, (
        "a stale lockfile must fail the gate before the alignment pass runs"
    )
