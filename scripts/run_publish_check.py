#!/usr/bin/env -S uv run python
# /// script
# requires-python = ">=3.11"
# dependencies = [
#     "tomlkit",
# ]
# ///
"""Run the publish-check workflow in a temporary workspace copy."""
from __future__ import annotations

import io
import json
import os
import shutil
import subprocess
import tarfile
import tempfile
import tomllib
from pathlib import Path

from publish_patch import REPLACEMENTS, apply_replacements

PUBLISH_CRATES = [
    "rstest-bdd-patterns",
    "rstest-bdd-macros",
    "rstest-bdd",
    "cargo-bdd",
]


def export_workspace(destination: Path) -> None:
    archive = subprocess.run(
        ["git", "archive", "--format=tar", "HEAD"],
        check=True,
        stdout=subprocess.PIPE,
    ).stdout
    with tarfile.open(fileobj=io.BytesIO(archive)) as tar:
        tar.extractall(destination, filter="data")


def strip_patch_section(manifest: Path) -> None:
    lines = manifest.read_text(encoding="utf-8").splitlines()
    cleaned: list[str] = []
    for line in lines:
        if line.strip() == "[patch.crates-io]":
            break
        cleaned.append(line)
    cleaned.append("")
    manifest.write_text("\n".join(cleaned), encoding="utf-8")


def prune_workspace_members(manifest: Path) -> None:
    lines = manifest.read_text(encoding="utf-8").splitlines()
    result: list[str] = []
    inside_members = False
    for line in lines:
        stripped = line.strip()
        if stripped.startswith("members") and stripped.endswith("["):
            inside_members = True
            result.append(line)
            continue
        if inside_members:
            if stripped == "]":
                inside_members = False
                result.append(line)
            elif '"crates/' in stripped:
                result.append(line)
            continue
        result.append(line)
    manifest.write_text("\n".join(result) + "\n", encoding="utf-8")


def apply_workspace_replacements(workspace_root: Path, version: str) -> None:
    for crate in REPLACEMENTS:
        manifest = workspace_root / "crates" / crate / "Cargo.toml"
        apply_replacements(crate, manifest, version)


def workspace_version(manifest: Path) -> str:
    data = tomllib.loads(manifest.read_text(encoding="utf-8"))
    return data["workspace"]["package"]["version"]


def package_crate(crate: str, workspace_root: Path) -> None:
    crate_dir = workspace_root / "crates" / crate
    env = dict(os.environ)
    env["CARGO_HOME"] = str(workspace_root / ".cargo-home")
    subprocess.run(
        ["cargo", "package", "--allow-dirty", "--no-verify"],
        check=True,
        cwd=crate_dir,
        env=env,
    )


def check_crate(crate: str, workspace_root: Path) -> None:
    crate_dir = workspace_root / "crates" / crate
    env = dict(os.environ)
    env["CARGO_HOME"] = str(workspace_root / ".cargo-home")
    subprocess.run(
        ["cargo", "check", "--all-features"],
        check=True,
        cwd=crate_dir,
        env=env,
    )


def main() -> None:
    workspace = Path(tempfile.mkdtemp())
    keep_workspace = bool(os.environ.get("PUBLISH_CHECK_KEEP_TMP"))
    try:
        export_workspace(workspace)
        manifest = workspace / "Cargo.toml"
        prune_workspace_members(manifest)
        strip_patch_section(manifest)
        version = workspace_version(manifest)
        apply_workspace_replacements(workspace, version)
        for crate in PUBLISH_CRATES:
            if crate == "rstest-bdd-patterns":
                package_crate(crate, workspace)
            else:
                check_crate(crate, workspace)
    finally:
        if keep_workspace:
            print(f"preserving workspace at {workspace}")
        else:
            shutil.rmtree(workspace, ignore_errors=True)


if __name__ == "__main__":
    main()
