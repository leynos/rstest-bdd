"""The guard that reads a nested workflow mapping.

`(step.get("env") or {})` accepts a non-empty scalar or list and then
raises `AttributeError` on the next read, which reports a fault in the
contract rather than the shape in the workflow that caused it. These
cover the guard that reports the shape instead.

Run via ``make test-workflow-contracts``.
"""

import pytest
from publish_report_support import mapping_at


@pytest.mark.parametrize(
    "value",
    [
        pytest.param("LADING_REF=abc", id="a-scalar"),
        pytest.param(["LADING_REF=abc"], id="a-list"),
        pytest.param(42, id="a-number"),
    ],
)
def test_a_malformed_nested_mapping_fails_as_a_shape_violation(
    value: object,
) -> None:
    """A scalar where a mapping belongs must name the workflow, not Python.

    ``(step.get("env") or {})`` accepts every value here and then raises
    ``AttributeError`` on the next read, which reports a fault in the
    contract rather than the shape in the workflow that caused it. A
    ``list`` is the realistic one: `env:` written as a sequence of
    `KEY=value` strings parses cleanly and means nothing to GitHub.
    """
    with pytest.raises(AssertionError, match="must be a mapping"):
        mapping_at({"env": value}, "env", "the step under test")


def test_an_absent_nested_mapping_reads_as_empty() -> None:
    """A step with no `env:` is ordinary, not malformed.

    The guard has to separate absent from wrong, or every step without
    the key would fail the contract instead of the assertions that read
    it reporting what is missing.
    """
    assert mapping_at({}, "env", "the step under test") == {}, (
        "a step with no env: must read as an empty mapping, or every such "
        "step fails the contract instead of the assertion that reads it"
    )
