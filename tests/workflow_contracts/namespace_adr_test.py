"""Protect the historical Whitaker update from Namespace migration drift.

Run with:

    pytest tests/workflow_contracts/namespace_adr_test.py
"""

from pathlib import Path

ADR_PATH = (
    Path(__file__).resolve().parents[2]
    / "docs"
    / "adr-013-adopt-whitaker-no-unwrap-or-else-panic.md"
)
HISTORICAL_UPDATE_HEADING = "## Update (2026-07-20): current compatibility contract"
NAMESPACE_ADDENDUM_HEADING = "## Addendum (2026-09-02): Namespace CI runner migration"


def _section_before(document: str, start_heading: str, end_heading: str) -> str:
    """Return the section bounded by two ordered headings."""
    start = document.find(start_heading)
    end = document.find(end_heading)
    assert start >= 0, f"missing documentation heading: {start_heading}"
    assert end > start, f"{end_heading} must follow {start_heading}"
    return document[start:end]


def test_namespace_contract_is_separate_from_historical_whitaker_update() -> None:
    """Keep current Namespace facts out of the historical compatibility record."""
    document = ADR_PATH.read_text(encoding="utf-8")
    historical_update = _section_before(
        document,
        HISTORICAL_UPDATE_HEADING,
        NAMESPACE_ADDENDUM_HEADING,
    )
    namespace_addendum = _section_before(
        document,
        NAMESPACE_ADDENDUM_HEADING,
        "## Known limitations",
    )

    assert "`whitaker-installer@0.2.6`" in historical_update, (
        "historical update must retain its original installer pin"
    )
    assert "Namespace" not in historical_update, (
        "Namespace migration facts belong only in the dated addendum"
    )

    for expected_contract in (
        "namespace-profile-rust-linux-ci",
        "namespace-profile-rust-windows-ci",
        "Ubuntu 24.04",
        "`whitaker-installer` at `0.2.7`",
        "install-whitaker",
        "GITHUB_PATH",
        "GNU Make",
        "contents: read",
        "make test-workflow-contracts",
        "all four `build-test` matrix lanes",
    ):
        assert expected_contract in namespace_addendum, (
            f"Namespace ADR addendum must record {expected_contract!r}"
        )
