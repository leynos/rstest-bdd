"""Prove the workflow loader refuses a key declared twice.

PyYAML keeps the last of two equal keys silently, so a lane can carry one
runner label in the text GitHub reads and another in the document the
contracts read. The loader is driven over documents carrying the duplicate,
and every workflow in the repository is loaded through it.

Run via ``make test-workflow-contracts``.
"""

import pytest
import yaml
from strict_workflow_loader import DuplicateKeyError, load_strict
from workflow_queries import workflow_names
from workflow_support import parse_workflow, repository_file

DUPLICATE_RUNS_ON = """\
jobs:
  build:
    runs-on: ubicloud-standard-2
    runs-on: ubuntu-latest
    steps: []
"""


def test_a_duplicate_key_is_refused_where_pyyaml_keeps_the_last() -> None:
    """Refuse the document that PyYAML reads as the second label alone."""
    discarded = yaml.safe_load(DUPLICATE_RUNS_ON)

    assert discarded["jobs"]["build"]["runs-on"] == "ubuntu-latest", (
        "PyYAML must be shown to keep the last value, or the case proves nothing"
    )
    with pytest.raises(DuplicateKeyError, match="runs-on"):
        load_strict(DUPLICATE_RUNS_ON)


def test_the_shared_parsing_boundary_refuses_it_too() -> None:
    """Refuse it through `parse_workflow`, which every shared reader uses.

    The loader proves nothing if the boundary stops calling it, and a parser
    fault converted to "not parsable" would hide which key repeated.
    """
    with pytest.raises(DuplicateKeyError):
        parse_workflow(DUPLICATE_RUNS_ON)


def test_the_bare_on_key_still_resolves_to_true() -> None:
    """Change nothing but duplicate handling.

    Every trigger rule relies on the bare word `on` parsing as `True`; a
    loader that stopped resolving it would move the trigger block to a key
    those rules no longer read.
    """
    assert load_strict("on: push\n") == {True: "push"}, (
        "the bare word on must still parse as True"
    )


def test_a_merge_key_may_restate_what_the_mapping_overrides() -> None:
    """Allow a merged mapping to supply a key the enclosing mapping restates."""
    text = "base: &base {a: 1}\nderived:\n  <<: *base\n  a: 2\n"

    document = load_strict(text)

    assert isinstance(document, dict), "the document must parse to a mapping"
    assert document["derived"] == {"a": 2}, (
        "a key restated over a merged mapping must override it, not fail"
    )


@pytest.mark.parametrize("name", workflow_names())
def test_every_workflow_declares_each_key_once(name: str) -> None:
    """Load every workflow strictly, whichever reader a contract uses.

    Some contracts read their workflow with `yaml.safe_load` directly. This
    rule holds the files themselves, so those readers cannot be handed a
    document that silently discarded a value.
    """
    load_strict(repository_file(".github", "workflows", name))
