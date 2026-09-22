"""Read a step's ``if:`` guard as the conjunction GitHub evaluates.

A contract asserting that a guard *contains* ``github.ref ==
'refs/heads/main'`` passes for ``... && github.ref == 'refs/heads/main' ||
github.event_name == 'workflow_dispatch'``, which makes every conjunct
optional and lets a dispatch from any branch through. The guards these
contracts reason about are therefore read as a list of conjuncts, and anything
this reader cannot represent as one, an unquoted ``||`` first among them, is
refused rather than approximated.

:func:`admits` then evaluates a guard against a named context, which is how
the contracts ask the behavioural question (does a dispatch from a feature
branch upload?) without a workflow runner. It evaluates only the conjunctive
equality subset these guards use and raises on anything else, so an unfamiliar
guard fails loudly instead of being judged by a reading that does not
understand it.
"""

import re
import typing as typ

from workflow_support import WorkflowShapeError

if typ.TYPE_CHECKING:
    import collections.abc as cabc

#: One conjunct: a context reference compared with a quoted literal, a bare
#: context reference read for truthiness, or a status function with no
#: arguments.
_COMPARISON = re.compile(
    r"^(?P<reference>[A-Za-z_][\w.-]*)\s*(?P<operator>==|!=)\s*'(?P<literal>[^']*)'$"
)
_REFERENCE = re.compile(r"^[A-Za-z_][\w.-]*$")
_STATUS_FUNCTIONS: typ.Final = frozenset({"always()", "success()"})


class UnsupportedGuardError(WorkflowShapeError):
    """A guard uses a form this reader will not approximate.

    Parameters
    ----------
    condition : str
        The guard as written.
    detail : str
        Which part could not be read.
    """

    def __init__(self, condition: str, detail: str) -> None:
        super().__init__(f"cannot read the guard {condition!r}: {detail}")


class UnknownContextError(WorkflowShapeError):
    """A guard reads a context value the caller did not supply.

    Parameters
    ----------
    reference : str
        The context reference.
    """

    def __init__(self, reference: str) -> None:
        super().__init__(
            f"the guard reads {reference!r}, which the evaluation context does "
            "not name; supply it rather than let it default"
        )


def _strip_expression(condition: str) -> str:
    """Remove one enclosing ``${{ }}`` and surrounding whitespace.

    Parameters
    ----------
    condition : str
        The guard as written.

    Returns
    -------
    str
        The bare expression.
    """
    bare = condition.strip()
    if bare.startswith("${{") and bare.endswith("}}"):
        bare = bare[3:-2].strip()
    return bare


def conjuncts(condition: str) -> list[str]:
    """Split a guard on ``&&`` and refuse any disjunction or grouping.

    Parameters
    ----------
    condition : str
        The guard as written, with or without ``${{ }}``.

    Returns
    -------
    list[str]
        Each conjunct, whitespace-normalized.

    Raises
    ------
    UnsupportedGuardError
        If the guard contains ``||``, a parenthesis, a negation, or an empty
        conjunct.

    Examples
    --------
    >>> conjuncts("${{ github.ref == 'refs/heads/main' && env.T != '' }}")
    ["github.ref == 'refs/heads/main'", "env.T != ''"]
    """
    bare = _strip_expression(condition)
    # Quoted literals may contain anything; `!=` is a comparison, not a
    # negation; and `()` closes a status call such as `always()`.
    scrubbed = re.sub(r"'[^']*'", "''", bare).replace("!=", "==").replace("()", "")
    for token, detail in (
        ("||", "a disjunction makes every conjunct optional"),
        ("(", "a grouped sub-expression"),
        (")", "an unmatched closing parenthesis"),
        ("!", "a negation"),
    ):
        if token in scrubbed:
            raise UnsupportedGuardError(condition, detail)
    parts = [" ".join(part.split()) for part in bare.split("&&")]
    if not all(parts):
        raise UnsupportedGuardError(condition, "an empty conjunct")
    return parts


def admits(condition: str, context: cabc.Mapping[str, str]) -> bool:
    """Evaluate a conjunctive guard against named context values.

    Comparison is case-insensitive, as GitHub's is for strings. A reference
    the context does not name raises instead of reading as empty, because the
    question being asked is only answered when every input to it is stated.
    Status functions are taken as true: the guards asserted on run after
    success, and ``always()`` is true by definition.

    Parameters
    ----------
    condition : str
        The guard as written; an empty guard admits everything.
    context : cabc.Mapping[str, str]
        Values for each reference, such as ``github.ref``.

    Returns
    -------
    bool
        Whether GitHub would run the step in that context.

    A conjunct that is not a comparison, a bare reference, or a status call
    fails with :class:`UnsupportedGuardError`, and one reading a reference the
    context does not name fails with :class:`UnknownContextError`.

    Examples
    --------
    >>> admits("github.ref == 'refs/heads/main'", {"github.ref": "refs/heads/x"})
    False
    """
    if not _strip_expression(condition):
        return True
    return all(_admits_one(part, condition, context) for part in conjuncts(condition))


def _admits_one(part: str, condition: str, context: cabc.Mapping[str, str]) -> bool:
    """Evaluate one conjunct.

    A reference the context does not name fails with
    :class:`UnknownContextError`, raised by :func:`_lookup`.

    Parameters
    ----------
    part : str
        The conjunct.
    condition : str
        The whole guard, for the error message.
    context : cabc.Mapping[str, str]
        The evaluation context.

    Returns
    -------
    bool
        The conjunct's value.

    Raises
    ------
    UnsupportedGuardError
        If the conjunct has no supported form.
    """
    if part in _STATUS_FUNCTIONS:
        return True
    if comparison := _COMPARISON.fullmatch(part):
        value = _lookup(comparison["reference"], context)
        equal = value.casefold() == comparison["literal"].casefold()
        return equal if comparison["operator"] == "==" else not equal
    if _REFERENCE.fullmatch(part):
        return _lookup(part, context).casefold() not in {"", "false", "0"}
    raise UnsupportedGuardError(condition, f"the conjunct {part!r}")


def _lookup(reference: str, context: cabc.Mapping[str, str]) -> str:
    """Return a context value, refusing one the caller did not supply.

    Parameters
    ----------
    reference : str
        The context reference.
    context : cabc.Mapping[str, str]
        The evaluation context.

    Returns
    -------
    str
        The supplied value.

    Raises
    ------
    UnknownContextError
        If the context does not name the reference.
    """
    if reference not in context:
        raise UnknownContextError(reference)
    return context[reference]
