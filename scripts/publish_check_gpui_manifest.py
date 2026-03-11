#!/usr/bin/env -S uv run python
"""Helpers for GPUI publish-check manifest generation and TOML rendering.

This module reads workspace and packaged crate metadata with ``tomllib`` and
renders the standalone manifests used by the GPUI publish-check workflow. It
accepts ``pathlib.Path`` inputs rooted in the workspace or extracted package
tree and returns TOML strings suitable for validator crates and synthetic
package archives. The helpers are pure string/manifest transforms and require
Python 3.11+ for ``tomllib``.

Example
-------
Import and call the helpers from the publish-check scripts:

>>> from pathlib import Path
>>> manifest = _packaged_manifest(
...     Path("/tmp/workspace"),
...     "0.5.0",
...     "rstest-bdd-harness-gpui",
... )
>>> "[package]" in manifest
True
>>> validator = _validator_manifest(
...     package_dir=Path("/tmp/package"),
...     harness_dir=Path("/tmp/harness"),
...     version="0.5.0",
...     validator_crate="gpui-validator",
... )
>>> "[patch.crates-io]" in validator
True
"""

from __future__ import annotations

import typing as typ

import tomllib

if typ.TYPE_CHECKING:
    from pathlib import Path


def _workspace_gpui_spec(workspace_root: Path) -> str:
    """Return the workspace ``gpui`` dependency as an inline TOML table string."""
    workspace = tomllib.loads(
        (workspace_root / "Cargo.toml").read_text(encoding="utf-8")
    )
    return _toml_inline_table(workspace["workspace"]["dependencies"]["gpui"])


def _validator_manifest(
    *,
    package_dir: Path,
    harness_dir: Path,
    version: str,
    validator_crate: str,
) -> str:
    """Return the manifest for the validator crate."""
    package_path = _toml_path(package_dir)
    harness_path = _toml_path(harness_dir)
    escaped_validator_crate = _escape_toml_string(validator_crate)
    escaped_version = _escape_toml_string(version)
    packaged_manifest = tomllib.loads(
        (package_dir / "Cargo.toml").read_text(encoding="utf-8")
    )
    gpui_spec = _toml_inline_table(packaged_manifest["dependencies"]["gpui"])
    return f"""[package]
name = "{escaped_validator_crate}"
version = "0.0.0"
edition = "2024"
publish = false

[workspace]

[dependencies]
gpui = {gpui_spec}
rstest-bdd-harness = "{escaped_version}"
rstest-bdd-harness-gpui = {{ path = "{package_path}" }}

[patch.crates-io]
rstest-bdd-harness = {{ path = "{harness_path}" }}
"""


def _packaged_manifest(workspace_root: Path, version: str, harness_crate: str) -> str:
    """Return the standalone manifest for the packaged GPUI harness crate."""
    workspace = tomllib.loads(
        (workspace_root / "Cargo.toml").read_text(encoding="utf-8")
    )
    crate = tomllib.loads(
        (workspace_root / "crates" / harness_crate / "Cargo.toml").read_text(
            encoding="utf-8"
        )
    )
    workspace_package = workspace["workspace"]["package"]
    gpui_spec = _workspace_gpui_spec(workspace_root)
    package = crate["package"]

    return """[package]
name = "{name}"
version = "{version}"
edition = "{edition}"
license = "{license}"
authors = {authors}
description = "{description}"
homepage = "{homepage}"
repository = "{repository}"
readme = "{readme}"
keywords = {keywords}
categories = {categories}
rust-version = "{rust_version}"

[lib]
doctest = false
test = false

[features]
native-gpui-tests = []

[dependencies]
rstest-bdd-harness = "{version}"
gpui = {gpui_spec}
""".format(
        name=_escape_toml_string(package["name"]),
        version=_escape_toml_string(version),
        edition=_escape_toml_string(workspace_package["edition"]),
        license=_escape_toml_string(workspace_package["license"]),
        authors=_toml_list(workspace_package["authors"]),
        description=_escape_toml_string(package["description"]),
        homepage=_escape_toml_string(workspace_package["homepage"]),
        repository=_escape_toml_string(workspace_package["repository"]),
        readme=_escape_toml_string(package["readme"]),
        keywords=_toml_list(workspace_package["keywords"]),
        categories=_toml_list(workspace_package["categories"]),
        rust_version=_escape_toml_string(workspace_package["rust-version"]),
        gpui_spec=gpui_spec,
    )


def _validator_test_source() -> str:
    """Return the smoke test source for the validator crate."""
    return """//! Smoke tests for the packaged GPUI harness artifact.

use rstest_bdd_harness::{
    HarnessAdapter, ScenarioMetadata, ScenarioRunRequest, ScenarioRunner,
};
use rstest_bdd_harness_gpui::GpuiHarness;

#[test]
fn packaged_gpui_harness_runs_against_upstream_gpui() {
    let request = ScenarioRunRequest::new(
        ScenarioMetadata::new(
            "tests/features/demo.feature",
            "Packaged GPUI harness",
            7,
            vec!["@ui".to_string()],
        ),
        ScenarioRunner::new(|context: gpui::TestAppContext| {
            context.test_function_name().is_none()
        }),
    );

    assert!(GpuiHarness::new().run(request));
}

#[gpui::test]
fn upstream_gpui_attribute_runs(context: &gpui::TestAppContext) {
    assert_eq!(context.test_function_name(), Some("upstream_gpui_attribute_runs"));
}
"""


def _toml_path(path: Path) -> str:
    """Return ``path`` as a POSIX string suitable for TOML manifests."""
    return _escape_toml_string(path.as_posix())


def _escape_toml_string(value: str) -> str:
    """Return ``value`` escaped for use in a TOML basic string."""
    return value.replace("\\", "\\\\").replace('"', '\\"')


def _toml_list(values: list[str]) -> str:
    """Return a TOML string-array literal for ``values``."""
    escaped_values = (_escape_toml_string(value) for value in values)
    quoted = ", ".join(f'"{value}"' for value in escaped_values)
    return f"[{quoted}]"


def _render_inline_table_value(*, key: str, value: object) -> str:
    """Return ``value`` rendered for a TOML inline table entry."""
    match value:
        case bool():
            return str(value).lower()
        case str():
            escaped_value = _escape_toml_string(value)
            return f'"{escaped_value}"'
        case list():
            casted_value = typ.cast("list[str]", value)
            return _toml_list(casted_value)
        case _:
            message = f"unsupported TOML inline-table value for {key!r}: {value!r}"
            raise SystemExit(message)


def _toml_inline_table(values: dict[str, object]) -> str:
    """Return ``values`` rendered as a TOML inline table."""
    rendered_items: list[str] = []
    for key, value in values.items():
        rendered_items.append(
            f"{key} = {_render_inline_table_value(key=key, value=value)}"
        )
    return "{ " + ", ".join(rendered_items) + " }"
