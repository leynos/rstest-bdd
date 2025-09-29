"""Manifest serialisation helpers shared across publish workspace utilities.

These utilities encapsulate TOML rendering behaviour so that individual
workflows can focus on their domain logic while relying on consistent output
formatting.
"""

from __future__ import annotations

import typing as typ
from pathlib import Path

from tomlkit import dumps

if typ.TYPE_CHECKING:
    from tomlkit.toml_document import TOMLDocument

__all__ = ["_write_manifest_with_newline"]


def _write_manifest_with_newline(document: TOMLDocument, manifest: Path) -> None:
    """Serialise ``document`` to ``manifest`` and ensure a trailing newline."""
    manifest = Path(manifest)
    rendered = dumps(document)
    if not rendered.endswith("\n"):
        rendered = f"{rendered}\n"

    manifest.write_text(rendered, encoding="utf-8")
