"""Unit tests for dependency patch helpers."""

from __future__ import annotations

import pytest
from publish_patch import build_inline_dependency, extract_existing_items
from tomlkit import inline_table, parse


class TestExtractExistingItems:
    """Tests for :func:`publish_patch.extract_existing_items`."""

    def test_filters_workspace_metadata(self) -> None:
        """Existing tables should discard workspace-specific keys."""
        document = parse(
            "[dependencies]\n"
            "foo = { workspace = true, path = '../foo', version = '0.1.0', "
            "default-features = false, features = ['serde'] }"
        )
        table = document["dependencies"]["foo"]

        assert extract_existing_items(table) == (
            ("default-features", False),
            ("features", ["serde"]),
        )

    def test_returns_empty_for_non_tables(self) -> None:
        """Non-table dependency declarations should produce no metadata."""
        assert extract_existing_items("version") == ()


class TestBuildInlineDependency:
    """Tests for :func:`publish_patch.build_inline_dependency`."""

    @pytest.mark.parametrize("include_local_path", [True, False])
    def test_builds_dependency_with_optional_path(
        self, *, include_local_path: bool
    ) -> None:
        """Inline dependencies should include the path only when requested."""
        extra_items = (("default-features", False),)

        inline = build_inline_dependency(
            extra_items,
            "../foo",
            "1.2.3",
            include_local_path=include_local_path,
        )

        expected = {"version": "1.2.3", "default-features": False}
        if include_local_path:
            expected["path"] = "../foo"

        assert dict(inline) == expected

    def test_appends_additional_metadata(self) -> None:
        """Existing metadata should be appended after the required fields."""
        extra_items = (
            ("default-features", False),
            ("features", ["serde"]),
        )

        inline = build_inline_dependency(
            extra_items,
            "../foo",
            "1.2.3",
            include_local_path=True,
        )

        assert list(inline.items()) == [
            ("path", "../foo"),
            ("version", "1.2.3"),
            ("default-features", False),
            ("features", ["serde"]),
        ]

    def test_accepts_inline_table_metadata(self) -> None:
        """The helper should accept iterables derived from inline tables."""
        existing = inline_table()
        existing["default-features"] = False
        extra_items = tuple(existing.items())

        inline = build_inline_dependency(
            extra_items,
            "../foo",
            "1.2.3",
            include_local_path=True,
        )

        assert dict(inline)["default-features"] is False
