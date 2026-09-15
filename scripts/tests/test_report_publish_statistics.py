"""Test the publish report reader's outcomes and its exit status.

Each way of having nothing is a different fault with a different remedy, so
the messages are asserted apart from one another: a report never written, one
created empty, one holding bytes that are not UTF-8, one truncated mid-write,
and one that is simply there. None of them may fail the job, because this
reports on a build rather than being one.
"""

import importlib
import json
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

    outcome = reader.read_report(absent)

    assert isinstance(outcome, reader.Unavailable), outcome
    assert str(absent) in outcome.reason, outcome.reason
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
