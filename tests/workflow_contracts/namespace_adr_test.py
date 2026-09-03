"""Protect the historical Whitaker update from Namespace migration drift.

Run with:

    pytest tests/workflow_contracts/namespace_adr_test.py
"""

from pathlib import Path

REPOSITORY_ROOT = Path(__file__).resolve().parents[2]
ADR_PATH = (
    REPOSITORY_ROOT / "docs" / "adr-013-adopt-whitaker-no-unwrap-or-else-panic.md"
)
HISTORICAL_BASELINE_PATH = (
    Path(__file__).resolve().parent / "data" / "adr-013-historical-update.md"
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


def test_historical_whitaker_update_matches_its_checked_in_baseline() -> None:
    """Hold the historical compatibility record byte-for-byte."""
    document = ADR_PATH.read_text(encoding="utf-8")
    historical_update = _section_before(
        document,
        HISTORICAL_UPDATE_HEADING,
        NAMESPACE_ADDENDUM_HEADING,
    )
    baseline = HISTORICAL_BASELINE_PATH.read_text(encoding="utf-8")
    assert historical_update == baseline, (
        "the 2026-07-20 compatibility record is a historical document; it must "
        f"stay byte-for-byte identical to {HISTORICAL_BASELINE_PATH.name}. "
        "Record current facts in the dated Namespace addendum instead."
    )


def test_namespace_addendum_records_the_current_runner_contract() -> None:
    """Keep current Namespace facts in the dated addendum."""
    document = ADR_PATH.read_text(encoding="utf-8")
    namespace_addendum = _section_before(
        document,
        NAMESPACE_ADDENDUM_HEADING,
        "## Known limitations",
    )

    for expected_contract in (
        "namespace-profile-rust-linux-ci",
        "namespace-profile-rust-windows-ci",
        "Ubuntu 24.04",
        "4 vCPU and 8 GB",
        "`whitaker-installer` at `0.2.7`",
        "install-whitaker",
        "GITHUB_PATH",
        "GNU Make",
        "contents: read",
        "make test-workflow-contracts",
        "all four `build-test` matrix lanes",
        "nscloud-cache-action",
        "allow_commit_from_branch",
    ):
        assert expected_contract in namespace_addendum, (
            f"Namespace ADR addendum must record {expected_contract!r}"
        )
