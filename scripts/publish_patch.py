#!/usr/bin/env python3
"""Utility helpers for adjusting manifests during publish checks."""
from __future__ import annotations

import argparse
from pathlib import Path

REPLACEMENTS = {
    "rstest-bdd-macros": {
        "rstest-bdd-patterns.workspace = true": 'rstest-bdd-patterns = {{ path = "../rstest-bdd-patterns", version = "{version}" }}',
        "rstest-bdd.workspace = true": 'rstest-bdd = {{ path = "../rstest-bdd", version = "{version}" }}',
    },
    "rstest-bdd": {
        "rstest-bdd-patterns.workspace = true": 'rstest-bdd-patterns = {{ path = "../rstest-bdd-patterns", version = "{version}" }}',
        "rstest-bdd-macros.workspace = true": 'rstest-bdd-macros = {{ path = "../rstest-bdd-macros", version = "{version}" }}',
    },
    "cargo-bdd": {
        'rstest-bdd = { workspace = true, features = ["diagnostics"] }': 'rstest-bdd = {{ path = "../rstest-bdd", version = "{version}", features = ["diagnostics"] }}',
    },
}


def apply_replacements(crate: str, manifest: Path, version: str) -> None:
    replacements = REPLACEMENTS[crate]
    text = manifest.read_text(encoding="utf-8")
    for old, template in replacements.items():
        new = template.format(version=version)
        if old not in text:
            raise SystemExit(f"expected {old!r} in {manifest}")
        text = text.replace(old, new)
    manifest.write_text(text, encoding="utf-8")


def main() -> None:
    parser = argparse.ArgumentParser(
        description="Adjust workspace manifests for publish-check packaging."
    )
    parser.add_argument("crate", choices=sorted(REPLACEMENTS))
    parser.add_argument("manifest", type=Path)
    parser.add_argument("--version", required=True)
    args = parser.parse_args()
    apply_replacements(args.crate, args.manifest, args.version)


if __name__ == "__main__":
    main()
