"""A YAML loader that refuses a mapping declaring one key twice.

PyYAML keeps the last of two equal keys and says nothing. A workflow that
declares ``runs-on`` twice therefore parses into a document that has silently
discarded the first value, so a lane can carry one label in the text GitHub
reads and another in the document every contract reads, and each contract
passes over a file that does something else. GitHub's own parser rejects the
duplicate, so the file would not run either; the defect is that the contracts
would not say so.

:func:`workflow_support.parse_workflow` loads through this module, so every
contract that reads a workflow through the shared boundary inherits the rule.

See Also
--------
workflow_support.parse_workflow : The parsing boundary built on this loader.
"""

import typing as typ

import yaml
from yaml.resolver import BaseResolver

#: The tag PyYAML gives a ``<<`` merge key. A merged mapping may legitimately
#: restate a key the enclosing mapping overrides, so only keys written in the
#: mapping itself are compared.
_MERGE_TAG: typ.Final[str] = "tag:yaml.org,2002:merge"


class DuplicateKeyError(AssertionError):
    """A mapping in a workflow declares the same key more than once.

    Derives from :class:`AssertionError`, as the contracts' shape errors do,
    so it reads as a failed expectation rather than a crash, and does not
    derive from :class:`yaml.YAMLError`, so a caller that turns parser faults
    into "not parsable" cannot swallow it into a vaguer message.

    Parameters
    ----------
    key : object
        The repeated key.
    line : int
        The one-based line of the second declaration.
    """

    def __init__(self, key: object, line: int) -> None:
        self.key = key
        self.line = line
        super().__init__(
            f"the key {key!r} is declared twice in one mapping (again at line "
            f"{line}); PyYAML would keep the last and discard the first silently"
        )


class StrictLoader(yaml.SafeLoader):
    """A safe loader whose mappings refuse duplicate keys."""


def _construct_unique_mapping(
    loader: StrictLoader, node: yaml.MappingNode, *, deep: bool = False
) -> dict[object, object]:
    """Build a mapping, refusing a key the same mapping has already declared.

    Parameters
    ----------
    loader : StrictLoader
        The loader constructing the document.
    node : yaml.MappingNode
        The mapping being constructed.
    deep : bool
        Whether nested values are constructed eagerly.

    Returns
    -------
    dict[object, object]
        The constructed mapping.

    Raises
    ------
    DuplicateKeyError
        If a key written in this mapping repeats an earlier one.
    """
    seen: set[object] = set()
    for key_node, _value_node in node.value:
        if key_node.tag == _MERGE_TAG:
            continue
        key = loader.construct_object(key_node, deep=True)
        try:
            is_repeat = key in seen
        except TypeError:
            # An unhashable complex key cannot collide with anything under
            # equality that PyYAML itself would honour; leave it to the
            # base constructor, which rejects it.
            continue
        if is_repeat:
            raise DuplicateKeyError(key, key_node.start_mark.line + 1)
        seen.add(key)
    return loader.construct_mapping(node, deep=deep)


StrictLoader.add_constructor(
    BaseResolver.DEFAULT_MAPPING_TAG, _construct_unique_mapping
)


def load_strict(text: str) -> object:
    """Parse YAML text, refusing any mapping that repeats a key.

    Resolves scalars as :func:`yaml.safe_load` does, so a bare ``on`` is still
    the boolean ``True``; only duplicate keys behave differently.

    Parameters
    ----------
    text : str
        The YAML document.

    Returns
    -------
    object
        The parsed document.

    A mapping that declares a key twice fails with :class:`DuplicateKeyError`,
    raised from the mapping constructor.

    Examples
    --------
    >>> load_strict("runs-on: a")
    {'runs-on': 'a'}
    """
    return yaml.load(text, Loader=StrictLoader)  # ruff: ignore[unsafe-yaml-load] - StrictLoader derives from SafeLoader.
