"""Drive the guard reader over conditions written for each form.

The contracts that evaluate a workflow's guards rest on this reader, and the
repository's own guards exercise only the forms they happen to use. Each
reading and each refusal is therefore proved against a condition carrying
exactly that form.

Run via ``make test-workflow-contracts``.
"""

import pytest
from guard_conditions import (
    UnknownContextError,
    UnsupportedGuardError,
    admits,
    conjuncts,
)

MAIN_PUSH = {
    "github.event_name": "push",
    "github.ref": "refs/heads/main",
    "env.CS_ACCESS_TOKEN": "set",
}


def test_a_folded_guard_splits_into_its_conjuncts() -> None:
    """Read a multi-line folded guard as the conjuncts GitHub evaluates."""
    condition = (
        "${{ runner.os == 'Windows'\n  && github.event_name == 'pull_request' }}"
    )

    expected = ["runner.os == 'Windows'", "github.event_name == 'pull_request'"]

    assert conjuncts(condition) == expected, f"{condition!r} must split into {expected}"


@pytest.mark.parametrize(
    "condition",
    [
        "github.ref == 'refs/heads/main' || github.event_name == 'workflow_dispatch'",
        "github.ref == 'refs/heads/main' && (env.A == 'x')",
        "!cancelled()",
        "github.ref == 'refs/heads/main' && ",
    ],
    ids=["disjunction", "group", "negation", "empty conjunct"],
)
def test_a_form_the_reader_cannot_represent_is_refused(condition: str) -> None:
    """Refuse rather than approximate.

    The disjunction is the case that matters: appended to a guard, it makes
    every conjunct optional while a substring check still finds the ref
    clause it was looking for.
    """
    with pytest.raises(UnsupportedGuardError):
        conjuncts(condition)


def test_operators_inside_a_quoted_literal_are_data() -> None:
    """Leave a literal alone even when it spells an operator."""
    assert conjuncts("env.A == '||(!)'") == ["env.A == '||(!)'"], (
        "operators inside a quoted literal must not be read as operators"
    )


@pytest.mark.parametrize(
    ("context", "expected"),
    [
        (MAIN_PUSH, True),
        ({**MAIN_PUSH, "github.ref": "refs/heads/feature"}, False),
        ({**MAIN_PUSH, "env.CS_ACCESS_TOKEN": ""}, False),
    ],
    ids=["main with a token", "another ref", "no token"],
)
def test_a_conjunction_is_evaluated_as_github_would(
    context: dict[str, str], expected: object
) -> None:
    """Evaluate each conjunct, and admit only when all of them hold."""
    condition = "${{ github.ref == 'refs/heads/main' && env.CS_ACCESS_TOKEN != '' }}"

    assert admits(condition, context) is expected, (
        f"{condition!r} must evaluate to {expected} in {context}"
    )


def test_string_comparison_ignores_case() -> None:
    """Compare strings the way GitHub's expression language does."""
    assert admits("runner.os == 'windows'", {"runner.os": "Windows"}), (
        "string comparison must ignore case, as GitHub's does"
    )


@pytest.mark.parametrize(
    ("value", "expected"), [("true", True), ("", False), ("false", False)]
)
def test_a_bare_reference_reads_as_truthiness(value: str, expected: object) -> None:
    """Read `matrix.tools` alone as GitHub reads a bare value."""
    assert admits("${{ matrix.tools }}", {"matrix.tools": value}) is expected, (
        f"a bare reference holding {value!r} must read as {expected}"
    )


def test_status_functions_and_an_empty_guard_admit() -> None:
    """Treat `always()` and a missing guard as running the step."""
    assert admits("${{ always() }}", {}), "always() must admit"
    assert admits("", {}), "a step with no guard must run"


def test_an_unnamed_reference_is_refused() -> None:
    """Refuse to default a context value the caller did not state.

    Reading it as empty would answer a question nobody asked: the evaluation
    is only meaningful when every input to it is named.
    """
    with pytest.raises(UnknownContextError):
        admits("github.ref == 'refs/heads/main'", {})
