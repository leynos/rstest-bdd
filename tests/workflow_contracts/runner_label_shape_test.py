"""Verify the shape of every declared runner label.

The Linux lane's label is a conditional expression, because a pull request
from a fork cannot obtain an Ubicloud runner. Two failures follow from that,
and neither shows up in a green run:

* a continuation indented one level deeper than its folded scalar keeps its
  line break, so the label carries a newline inside the expression and GitHub
  evaluates it regardless; and
* a step keyed on the literal label switches off on whichever arm it did not
  name, silently skipping work the lane is there to do.

Run with:

    pytest tests/workflow_contracts/runner_label_shape_test.py
"""

import pytest
from runner_label_support import FORK_FIELD
from runner_label_support import (
    literal_label_guard as _literal_label_guard,
)
from runner_label_support import (
    runner_label_expression as _runner_label_expression,
)
from runner_label_support import (
    runner_labels as _runner_labels,
)
from workflow_support import GITHUB_HOSTED_LINUX, UBICLOUD_LINUX_LABEL
from workflow_support import (
    job as _job,
)
from workflow_support import (
    steps as _steps,
)

#: Where the fork fallback is declared. Named once so the two contracts that
#: read it cannot drift apart.
LINUX_LANE_SOURCE = "strategy.matrix.include[0].os"


def test_no_declared_runner_label_spans_lines() -> None:
    """Refuse a label a folded scalar broke across lines.

    A more-indented continuation puts a literal newline inside the
    expression. The document still parses, actionlint still passes and
    GitHub still evaluates the expression, so only the raw text says
    the declaration is wrong.
    """
    broken = [
        f"{label.where} declares {label.raw!r}"
        for label in _runner_labels()
        if "\n" in label.raw
    ]
    assert not broken, (
        "a runner label must parse to a single line; a continuation indented "
        "deeper than its folded scalar keeps its line break and puts a "
        f"newline inside the expression: {broken}"
    )


def test_linux_lane_falls_back_to_github_hosted_for_forks() -> None:
    """Send fork pull requests to GitHub-hosted Linux, everything else to Ubicloud.

    The guard is asserted as the whole field path: a sibling field of the same
    object, such as the head repository's ``private`` flag, reads almost
    identically and would route every fork pull request to a runner it can
    never obtain.
    """
    labels = {
        label.source: label for label in _runner_labels() if label.workflow == "ci.yml"
    }
    declared = labels.get(LINUX_LANE_SOURCE)
    assert declared is not None, (
        f"ci.yml:build-test must declare its Linux lane at {LINUX_LANE_SOURCE}"
    )
    label = _runner_label_expression(declared.raw)
    assert label.guard == FORK_FIELD, (
        f"the Linux lane must branch on {FORK_FIELD}, which is the only field "
        f"that says a runner cannot be obtained; got {label.guard!r}"
    )
    assert label.when_true == GITHUB_HOSTED_LINUX, (
        f"a fork pull request must fall back to {GITHUB_HOSTED_LINUX}; got "
        f"{label.when_true!r}"
    )
    assert label.when_false == UBICLOUD_LINUX_LABEL, (
        f"every other event must reach {UBICLOUD_LINUX_LABEL}; got {label.when_false!r}"
    )


def test_no_step_is_keyed_on_the_literal_runner_label() -> None:
    """Key conditional steps on the runner, not on the label that resolved it.

    ``matrix.os`` now resolves to whichever arm the event selected, so a step
    guarded by one literal label stops running on the other arm without
    failing anything.
    """
    keyed = [
        f"{step.get('name', '<unnamed>')!r} {reason} in {str(step.get('if')).strip()!r}"
        for step in _steps(_job("ci.yml", "build-test"))
        if (reason := _literal_label_guard(str(step.get("if", "")))) is not None
    ]
    assert not keyed, (
        "a step in a lane whose label is an expression must key on "
        "runner.os, not on the literal label it happened to resolve to; "
        f"found {keyed}"
    )


@pytest.mark.parametrize(
    "condition",
    [
        pytest.param("${{ matrix.os == 'ubicloud-standard-2' }}", id="equality"),
        pytest.param("${{ matrix.os != 'ubuntu-latest' }}", id="inequality"),
        pytest.param("${{ 'ubicloud-standard-2' == matrix.os }}", id="reversed"),
        pytest.param(
            "${{ contains(matrix.os, 'ubicloud') }}", id="containment-on-matrix-os"
        ),
        pytest.param(
            "${{ runner.environment == 'self-hosted' && "
            "github.workflow != 'ubuntu-latest' }}",
            id="label-named-without-matrix-os",
        ),
    ],
)
def test_literal_label_guard_refuses_every_spelling(condition: str) -> None:
    """Refuse a label-keyed condition whichever way it is written.

    ``ci.yml`` contains none of these, so reading the rule off the workflow
    would pass whether it discriminated or not. The rule is driven directly
    instead. An operator-matching rule accepted the last four
    of these while each one still skips a step on one arm of the lane.
    """
    assert _literal_label_guard(condition) is not None, (
        f"{condition!r} keys a step on the resolved runner label and must be "
        "refused however the comparison is spelled"
    )


@pytest.mark.parametrize(
    "condition",
    [
        pytest.param("", id="unconditional"),
        pytest.param("${{ runner.os == 'Linux' }}", id="runner-os-linux"),
        pytest.param(
            "${{ runner.os == 'Linux' && github.event_name == 'pull_request' }}",
            id="runner-os-composed",
        ),
        pytest.param(
            "${{ runner.os == 'Windows' && matrix.features == '' }}",
            id="matrix-sibling",
        ),
        pytest.param("${{ matrix.tools }}", id="matrix-tools"),
    ],
)
def test_literal_label_guard_admits_runner_keyed_conditions(condition: str) -> None:
    """Admit the conditions the lane actually needs.

    A rule that refused every condition would satisfy the contract above
    while making the workflow unwritable, so the admitted cases are asserted
    too: ``runner.os``, a sibling matrix value, and no condition at all.
    """
    assert _literal_label_guard(condition) is None, (
        f"{condition!r} is keyed on the runner, not on a label, and must be admitted"
    )
