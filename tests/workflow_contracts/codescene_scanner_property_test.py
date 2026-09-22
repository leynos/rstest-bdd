"""Properties of the CodeScene scan, and its readers' failure paths.

The fixed fixtures in :mod:`codescene_coverage_test` put each marker where a
real workflow would. These properties put it anywhere: at any depth, under a
mapping key or in a list, as a key or as a value, inside surrounding text. A
traversal defect at a shape the fixtures do not use would pass them and fail
here. Marker-free trees are generated too, because a scan that reported
everything would pass the first property.

Run via ``make test-workflow-contracts``.
"""

import typing as typ

import hypothesis
import pytest
from codescene_coverage_support import (
    MARKERS,
    PR_WORKFLOW,
    MissingCoverageStepError,
    MissingStepInputsError,
    MissingTriggerBlockError,
    coverage_step,
    references_in,
    trigger_mapping,
)
from coverage_lane_pairs import GATE_JOB
from hypothesis import strategies as st

if typ.TYPE_CHECKING:
    import collections.abc as cabc

#: Text that can carry no marker: lower-case letters and digits never spell
#: `codescene`, `cs-coverage` or `cs_access_token`, and no `inherit` can sit
#: under a `secrets` key because no generated key is `secrets`.
_INNOCENT_TEXT = st.text(alphabet="abdfghjklmnpqrtuvwxyz0123456789 ", max_size=12)
_INNOCENT_TREES = st.recursive(
    _INNOCENT_TEXT | st.integers() | st.booleans() | st.none(),
    lambda children: (
        st.lists(children, max_size=4)
        | st.dictionaries(_INNOCENT_TEXT, children, max_size=4)
    ),
    max_leaves=20,
)
#: A path into a tree: each step is either a mapping key or a list position.
_PATHS = st.lists(
    st.one_of(_INNOCENT_TEXT.map(lambda key: ("key", key)), st.just(("item", None))),
    max_size=6,
)


def _plant(
    marker_text: str, path: cabc.Sequence[tuple[str, str | None]], *, as_key: bool
) -> object:
    """Build a tree holding ``marker_text`` at the end of ``path``.

    Parameters
    ----------
    marker_text : str
        The scalar carrying the marker.
    path : cabc.Sequence[tuple[str, str | None]]
        The containers to wrap it in, outermost first.
    as_key : bool
        Whether the marker is written as a mapping key rather than a value.

    Returns
    -------
    object
        The document.
    """
    tree: object = {marker_text: None} if as_key else marker_text
    for kind, key in reversed(path):
        tree = {key: tree} if kind == "key" else [tree]
    return tree


@st.composite
def _planted(draw: st.DrawFn) -> tuple[str, object]:
    """Draw a marker and a document carrying it somewhere.

    Returns
    -------
    tuple[str, object]
        The marker's description and the document it was planted in.
    """
    marker = draw(st.sampled_from(sorted(MARKERS)))
    text = f"{draw(_INNOCENT_TEXT)}{MARKERS[marker]}{draw(_INNOCENT_TEXT)}"
    return marker, _plant(text, draw(_PATHS), as_key=draw(st.booleans()))


@hypothesis.settings(max_examples=200, deadline=None)
@hypothesis.given(planted=_planted())
def test_a_marker_is_found_wherever_it_is_written(planted: tuple[str, object]) -> None:
    """Find every marker at any depth, in any container, as key or value.

    No deadline: the scan is pure, but a loaded host stretches wall time and
    the property has no timing component.
    """
    marker, document = planted

    found = references_in(document, "doc")

    assert any(entry.startswith(marker) for entry in found), (
        f"{marker} planted in {document!r} must be found; found {found}"
    )


@hypothesis.settings(max_examples=200, deadline=None)
@hypothesis.given(document=_INNOCENT_TREES)
def test_a_tree_with_no_marker_is_cleared(document: object) -> None:
    """Report nothing for a tree that carries no interaction."""
    found = references_in(document, "doc")

    assert found == [], f"{document!r} carries no marker; the scan found {found}"


def test_a_list_trigger_block_is_refused_where_a_mapping_is_needed() -> None:
    """Refuse the forms that carry no filters where filters are read."""
    with pytest.raises(MissingTriggerBlockError):
        trigger_mapping({True: ["push"]}, "fixture.yml")


def test_an_absent_coverage_step_is_named() -> None:
    """Name the job and the steps it has, rather than returning nothing."""
    with pytest.raises(MissingCoverageStepError, match=GATE_JOB):
        coverage_step(PR_WORKFLOW, "No such step", GATE_JOB)


def test_a_step_without_inputs_is_refused() -> None:
    """Refuse a named step that configures no action."""
    with pytest.raises(MissingStepInputsError):
        coverage_step(PR_WORKFLOW, "Free disk space", GATE_JOB)
