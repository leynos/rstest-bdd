"""Drive the pull-request closure over documents written for each reading.

Nothing in this repository calls a local reusable workflow, and every workflow
declares its triggers in the mapping form, so read from `.github/workflows` a
reader that followed no call and understood one trigger form would pass. Each
rule here is therefore proved against a document carrying exactly the shape it
is about.

Run via ``make test-workflow-contracts``.
"""

import pytest
from codescene_coverage_support import pull_request_workflows, references_in
from pull_request_reach import (
    MissingCalledWorkflowError,
    TriggerShapeError,
    UnrecognizedCallError,
    calls_in,
    pull_request_closure,
    trigger_names,
)

#: Every form GitHub accepts for a trigger block, under the key PyYAML
#: produces for the bare word `on` and under the quoted string.
TRIGGER_FORMS = {
    "a scalar": ("pull_request", {"pull_request"}),
    "a list": (["push", "pull_request"], {"push", "pull_request"}),
    "a mapping": (
        {"push": {"branches": ["main"]}, "pull_request": None},
        {"push", "pull_request"},
    ),
}


@pytest.mark.parametrize("key", [True, "on"], ids=["boolean key", "string key"])
@pytest.mark.parametrize("form", sorted(TRIGGER_FORMS))
def test_every_trigger_form_is_read(form: str, key: object) -> None:
    """Read the scalar, list and mapping forms under either key.

    A mapping-only reader stringifies `on: [push, pull_request]` into one key
    named after the whole list, so that workflow escapes every pull-request
    rule while the others keep the suite green.
    """
    declared, expected = TRIGGER_FORMS[form]

    found = trigger_names({key: declared})

    assert found == expected, (
        f"{form} under {key!r} must read as {expected}; got {found}"
    )


@pytest.mark.parametrize(
    "document",
    [
        {"jobs": {}},
        {True: "push", "on": "pull_request"},
        {True: None},
        {True: []},
        {True: [{"push": None}]},
        {True: 3},
    ],
    ids=["absent", "both keys", "empty", "empty list", "list of mappings", "number"],
)
def test_an_unreadable_trigger_block_is_refused(document: dict[object, object]) -> None:
    """Refuse a trigger block rather than read it as declaring nothing.

    A workflow read as having no triggers is cleared by every pull-request
    rule, so an unreadable block has to fail rather than default to empty.
    """
    with pytest.raises(TriggerShapeError):
        trigger_names(document)


def _caller(reference: str) -> dict[str, object]:
    """Return a workflow whose only job calls ``reference``."""
    return {"jobs": {"call": {"uses": reference}}}


@pytest.mark.parametrize(
    "reference",
    [
        "./.github/workflows/called.yml",
        ".github/workflows/called.yml",
        "./.github/workflows/../workflows/called.yml",
        "leynos/rstest-bdd/.github/workflows/called.yml@main",
    ],
)
def test_a_same_repository_call_is_followed_by_shape(reference: str) -> None:
    """Follow any spelling that resolves to a file in the workflow directory.

    Matched by what the path resolves to rather than by a list of prefixes,
    so a spelling nobody enumerated is still followed.
    """
    found = calls_in(_caller(reference), {"called.yml"})

    assert found == {"called.yml"}, f"{reference!r} must be followed; got {found}"


def test_a_remote_reusable_workflow_is_not_followed() -> None:
    """Leave another repository's workflow to the caller's `secrets:` scan.

    Its text is not here to read. What it can receive is decided by the
    caller's `secrets:` block, which the CodeScene scan reads.
    """
    reference = "leynos/shared-actions/.github/workflows/x.yml@" + "0" * 40

    found = calls_in(_caller(reference), {"x.yml"})

    assert found == frozenset(), f"a remote call must not be followed; got {found}"


def test_a_step_level_uses_is_an_action_not_a_call() -> None:
    """Follow job-level calls only.

    A step's `uses` runs an action, not a workflow, so treating it as a call
    would grow the closure with files no event runs.
    """
    document = {"jobs": {"a": {"steps": [{"uses": "./.github/workflows/called.yml"}]}}}

    found = calls_in(document, {"called.yml"})

    assert found == frozenset(), f"a step-level uses is not a call; got {found}"


@pytest.mark.parametrize(
    "reference",
    ["$/.github/workflows/called.yml", "../elsewhere/called.yml", "called.yml"],
)
def test_an_unrecognized_call_is_refused(reference: str) -> None:
    """Fail on a call this reader cannot place.

    Skipping it would drop the called workflow from the closure in silence,
    which is exactly how an unenumerated spelling escaped before.
    """
    with pytest.raises(UnrecognizedCallError):
        calls_in(_caller(reference), {"called.yml"})


def test_a_call_to_a_missing_workflow_is_refused() -> None:
    """Refuse to compute a closure over a file that is not there."""
    with pytest.raises(MissingCalledWorkflowError):
        calls_in(_caller("./.github/workflows/absent.yml"), {"called.yml"})


def test_the_closure_follows_calls_transitively() -> None:
    """Reach a workflow only a called workflow calls, and nothing else.

    A chain of two calls is the shortest case one hop would miss, and the
    leaf declares only `workflow_call`, so a trigger list cannot see it.
    """
    documents = {
        "entry.yml": {
            True: {"pull_request": None},
            **_caller("./.github/workflows/middle.yml"),
        },
        "middle.yml": {
            True: "workflow_call",
            **_caller("./.github/workflows/leaf.yml"),
        },
        "leaf.yml": {
            True: "workflow_call",
            "jobs": {"c": {"steps": [{"run": "true"}]}},
        },
        "unrelated.yml": {True: {"schedule": None}, "jobs": {}},
    }

    reached = pull_request_closure(documents)

    assert reached == {"entry.yml", "middle.yml", "leaf.yml"}, (
        f"the closure must reach the chain and nothing else; got {sorted(reached)}"
    )


def test_the_closure_reaches_a_called_workflows_codescene_contact() -> None:
    """Scan what a pull request can run, not what it names in its triggers.

    The probe that measured the hole in another repository: a workflow that
    declares only `workflow_call`, called from a pull-request job with
    `secrets: inherit`, curling the CodeScene API with the inherited token.
    Enumerating triggers, the called workflow is not a pull-request workflow
    at all, so every clause passes over it.
    """
    documents = {
        "gate.yml": {
            True: {"pull_request": None},
            "jobs": {
                "call": {
                    "uses": "./.github/workflows/helper.yml",
                    "secrets": "inherit",
                }
            },
        },
        "helper.yml": {
            True: {"workflow_call": None},
            "jobs": {
                "probe": {
                    "steps": [
                        {
                            "run": (
                                "curl -H 'Authorization: Bearer "
                                "${{ secrets.CS_ACCESS_TOKEN }}' "
                                "https://api.codescene.io/v2/projects"
                            )
                        }
                    ]
                }
            },
        },
    }

    reached = pull_request_closure(documents)
    found = {name: references_in(documents[name], name) for name in reached}

    assert reached == {"gate.yml", "helper.yml"}, (
        f"the closure must include the called workflow; it reached {sorted(reached)}"
    )
    assert found["helper.yml"], "the called workflow's CodeScene contact must be found"
    assert found["gate.yml"], "the caller's secrets: inherit must be found"


def test_the_boundary_query_scans_the_closure() -> None:
    """Drive the composition the boundary rule calls, not only its parts.

    This repository calls no local reusable workflow, so read from its own
    files a query that had fallen back to the trigger list would pass.
    """
    documents = {
        "gate.yml": {
            True: {"pull_request": None},
            "jobs": {"call": {"uses": "./.github/workflows/helper.yml"}},
        },
        "helper.yml": {True: {"workflow_call": None}, "jobs": {}},
    }

    reached = pull_request_workflows(documents)

    assert reached == ["gate.yml", "helper.yml"], (
        f"the boundary must scan the called workflow too; it scans {reached}"
    )
