"""Verify what :func:`cache_step_support.cache_paths` accepts and rejects.

The cache contracts count what a cache step owns by reading its declared
paths, so a declaration that names nothing has to be reported rather than
read as a step owning no paths: an empty list would satisfy every
ownership, placement, and save-policy assertion without any of them
examining the step.

Run with:

    pytest tests/workflow_contracts/cache_step_support_test.py
"""

import pytest
from cache_step_support import cache_paths as _cache_paths
from workflow_support import CacheStepPathsError


@pytest.mark.parametrize("declared", ["", "   ", "\n\t\n"])
def test_a_path_naming_nothing_is_reported(declared: str) -> None:
    """A blank declaration must raise, not read as owning no paths."""
    with pytest.raises(CacheStepPathsError):
        _cache_paths({"with": {"path": declared}})
