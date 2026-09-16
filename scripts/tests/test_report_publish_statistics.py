"""Test the publish report reader's outcomes and its exit status.

Each way of having nothing is a different fault with a different remedy, so
the messages are asserted apart from one another: a report never written, one
created empty, one holding bytes that are not UTF-8, one truncated mid-write,
and one that is simply there. None of them may fail the job, because this
reports on a build rather than being one.
"""

import importlib
import json
import os
import typing as typ
from pathlib import Path

import pytest

if typ.TYPE_CHECKING:
    import types

SCRIPTS = Path(__file__).resolve().parents[1]


@pytest.fixture(name="reader")
def reader_module() -> types.ModuleType:
    """Return the reader module, imported from the scripts directory.

    Returns
    -------
    types.ModuleType
        The imported ``report_publish_statistics`` module.
    """
    import sys

    if str(SCRIPTS) not in sys.path:
        sys.path.insert(0, str(SCRIPTS))
    return importlib.import_module("report_publish_statistics")


def run_main(
    reader: types.ModuleType,
    monkeypatch: pytest.MonkeyPatch,
    stats_path: Path | None,
) -> int:
    """Run the reader with ``STATS_PATH`` set, or deliberately unset.

    Parameters
    ----------
    reader : types.ModuleType
        The module under test.
    monkeypatch : pytest.MonkeyPatch
        Used to set or remove the environment variable.
    stats_path : Path or None
        The report path to advertise, or ``None`` to leave it unset.

    Returns
    -------
    int
        The exit status the reader returned.
    """
    if stats_path is None:
        monkeypatch.delenv(reader.STATS_PATH_VARIABLE, raising=False)
    else:
        monkeypatch.setenv(reader.STATS_PATH_VARIABLE, str(stats_path))
    return reader.main()


def test_a_missing_report_names_the_path_and_a_cause(
    reader: types.ModuleType,
    monkeypatch: pytest.MonkeyPatch,
    capsys: pytest.CaptureFixture[str],
    tmp_path: Path,
) -> None:
    """The common case: the publish step failed before lading ran.

    The artefact list looks the same whether the report was never written
    or was written and never read, so the warning has to carry the
    annotation GitHub renders, a cause to act on, and the path to look for.
    """
    absent = tmp_path / "sccache-publish.json"

    assert run_main(reader, monkeypatch, absent) == 0, (
        "the reader must not fail the job over a report it could not find"
    )

    printed = capsys.readouterr().out
    expected = (
        f"::warning title={reader.WARNING_TITLE}::",
        "wrote no compiler-cache report",
        str(absent),
    )
    missing = [fragment for fragment in expected if fragment not in printed]
    assert not missing, f"missing {missing} from {printed!r}"


def test_an_empty_report_is_distinguished_from_a_missing_one(
    reader: types.ModuleType,
    monkeypatch: pytest.MonkeyPatch,
    capsys: pytest.CaptureFixture[str],
    tmp_path: Path,
) -> None:
    """Lading created the file and wrote nothing to it.

    Collapsing this into the missing-report message would send a reader
    looking for a publish failure that did not happen.
    """
    empty = tmp_path / "sccache-publish.json"
    empty.write_bytes(b"")

    assert run_main(reader, monkeypatch, empty) == 0, (
        "an empty report is a diagnosis, not a build failure"
    )

    printed = capsys.readouterr().out
    assert "is empty" in printed, printed
    assert "wrote no compiler-cache report" not in printed, printed


def test_a_report_of_undecodable_bytes_warns_rather_than_raising(
    reader: types.ModuleType,
    monkeypatch: pytest.MonkeyPatch,
    capsys: pytest.CaptureFixture[str],
    tmp_path: Path,
) -> None:
    """A report that is not UTF-8 text reaches the read, not the parse.

    ``read_text`` raises before any JSON parsing happens, so the
    unparsable-report branch cannot cover this. Unguarded, the exception
    escapes the reader and takes the verification step, and with it the
    lane, down over a diagnostic the lane does not depend on.
    """
    undecodable = tmp_path / "sccache-publish.json"
    undecodable.write_bytes(b"\xff\xfe{}")

    assert run_main(reader, monkeypatch, undecodable) == 0, (
        "an unreadable report is a diagnosis, not a build failure"
    )

    printed = capsys.readouterr().out
    assert "could not be read" in printed, printed
    assert f"::warning title={reader.WARNING_TITLE}::" in printed, printed


def test_a_truncated_report_is_reported_as_unreadable(
    reader: types.ModuleType,
    monkeypatch: pytest.MonkeyPatch,
    capsys: pytest.CaptureFixture[str],
    tmp_path: Path,
) -> None:
    """A partial write leaves a non-empty file that is not JSON.

    Testing only for existence would pass this, which is why the reader
    parses rather than stats.
    """
    truncated = tmp_path / "sccache-publish.json"
    truncated.write_text('{"requests": ', encoding="utf-8")

    assert run_main(reader, monkeypatch, truncated) == 0, (
        "a truncated report is a diagnosis, not a build failure"
    )

    printed = capsys.readouterr().out
    assert "not valid JSON" in printed, printed


def test_a_valid_report_is_printed_rather_than_warned_about(
    reader: types.ModuleType,
    monkeypatch: pytest.MonkeyPatch,
    capsys: pytest.CaptureFixture[str],
    tmp_path: Path,
) -> None:
    """The successful case, which must produce no warning at all.

    A reader that warned unconditionally would satisfy all three cases
    above and tell a maintainer nothing.
    """
    report = tmp_path / "sccache-publish.json"
    report.write_text(json.dumps({"delta": {"hits": 17, "misses": 15}}), "utf-8")

    assert run_main(reader, monkeypatch, report) == 0, (
        "a readable report must leave the job alive"
    )

    printed = capsys.readouterr().out
    assert "::warning" not in printed, printed
    assert "Publish-step compiler-cache report:" in printed, printed
    assert '"hits": 17' in printed, printed


def test_an_unset_path_variable_warns_rather_than_raising(
    reader: types.ModuleType,
    monkeypatch: pytest.MonkeyPatch,
    capsys: pytest.CaptureFixture[str],
) -> None:
    """A workflow edit could drop the variable from the step's environment.

    Reading the empty string as a path would report a nonsense location;
    naming the variable sends the reader to the step that should set it.
    """
    assert run_main(reader, monkeypatch, None) == 0, (
        "an unset variable must warn rather than raise"
    )

    printed = capsys.readouterr().out
    assert reader.STATS_PATH_VARIABLE in printed, printed


def test_the_query_returns_its_reason_instead_of_announcing_it(
    reader: types.ModuleType,
    capsys: pytest.CaptureFixture[str],
    tmp_path: Path,
) -> None:
    """Reading decides; only ``main`` announces.

    The reason has to survive as a value, because a query that warned on its
    own behalf would emit a second annotation whenever a caller wanted to
    handle the outcome itself, and its reason could then only be asserted by
    scraping captured output.
    """
    absent = tmp_path / "sccache-publish.json"

    match reader.read_report(absent):
        case reader.Unavailable(reason=reason):
            assert str(absent) in reason, reason
        case unexpected:
            pytest.fail(f"expected the reason, got the report {unexpected!r}")
    announced = capsys.readouterr().out
    assert not announced, f"the query must announce nothing, got {announced!r}"


def test_the_query_returns_the_report_text_unchanged(
    reader: types.ModuleType,
    tmp_path: Path,
) -> None:
    """A readable report comes back as text, not wrapped in an outcome type.

    The success and failure arms are told apart by type, so the caller needs
    no sentinel and cannot mistake a report whose contents happen to look
    like a reason for a failure.
    """
    report = tmp_path / "sccache-publish.json"
    body = json.dumps({"delta": {"hits": 17, "misses": 15}})
    report.write_text(body, encoding="utf-8")

    assert reader.read_report(report) == body, (
        "a readable report must come back as its own text"
    )


def permission_bits_bite() -> bool:
    """Report whether mode bits can actually deny this process a read.

    Two things stop them. On Windows ``chmod`` only toggles a read-only flag,
    so a mode-000 file stays readable and a mode-000 directory stays
    traversable. On POSIX, root ignores the bits outright. Where neither
    holds, the faults these cases provoke cannot be provoked at all.

    Returns
    -------
    bool
        True when a mode-000 path is genuinely unreadable here.
    """
    return os.name == "posix" and os.geteuid() != 0


#: ``os.geteuid`` does not exist on Windows, so the platform test has to come
#: first and the whole decision has to be made in a function rather than in a
#: bare expression evaluated at import time.
unprivileged_posix_only = pytest.mark.skipif(
    not permission_bits_bite(),
    reason="mode bits cannot deny a read to root, or on Windows",
)


@unprivileged_posix_only
def test_an_unreadable_report_is_not_reported_as_a_missing_one(
    reader: types.ModuleType,
    monkeypatch: pytest.MonkeyPatch,
    capsys: pytest.CaptureFixture[str],
    tmp_path: Path,
) -> None:
    """A report that exists but cannot be read is its own fault.

    Collapsing it into the missing-report message would send a reader looking
    for a publish failure that did not happen, which is the same confusion the
    empty-report case exists to prevent. The lane must still survive it.
    """
    unreadable = tmp_path / "sccache-publish.json"
    unreadable.write_text('{"delta": {}}', encoding="utf-8")
    unreadable.chmod(0o000)

    try:
        assert run_main(reader, monkeypatch, unreadable) == 0, (
            "an unreadable report is a diagnosis, not a build failure"
        )
    finally:
        unreadable.chmod(0o600)

    printed = capsys.readouterr().out
    assert "could not be read" in printed, printed
    assert "wrote no compiler-cache report" not in printed, printed


@unprivileged_posix_only
def test_a_report_behind_an_unreachable_directory_is_not_reported_as_missing(
    reader: types.ModuleType,
    tmp_path: Path,
) -> None:
    """The case that motivated dropping the separate existence check.

    ``Path.is_file()`` answers False for a path it cannot reach, so an
    existence check ahead of the read cannot tell an unreachable report from
    an absent one, and would announce the wrong cause with full confidence.
    """
    parent = tmp_path / "locked"
    parent.mkdir()
    hidden = parent / "sccache-publish.json"
    hidden.write_text('{"delta": {}}', encoding="utf-8")
    parent.chmod(0o000)

    try:
        outcome = reader.read_report(hidden)
    finally:
        parent.chmod(0o700)

    match outcome:
        case reader.Unavailable(reason=reason):
            assert "could not be read" in reason, reason
        case unexpected:
            pytest.fail(f"expected the reason, got the report {unexpected!r}")
