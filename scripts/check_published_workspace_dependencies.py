#!/usr/bin/env python3
"""Check staged package manifests use current internal release lower bounds.

Cargo removes local ``path`` dependencies from a package manifest before it
verifies the tarball. A publishable workspace crate that consumes a new public
API must therefore require the current unified workspace version of that
internal dependency. This gate inspects the generated manifests staged by
``make stage-published-gpui-e2e`` and fails before the release dry run when a
lower bound would resolve to an older crates.io package.
"""

import argparse
import re
import sys
import tomllib
import typing as typ
from pathlib import Path

if typ.TYPE_CHECKING:
    import collections.abc as cabc

VERSION_PATTERN = re.compile(
    r"^\s*(?P<operator>\^|~|=|>=|<=|<|>)?\s*"
    r"(?P<major>\d+)(?:\.(?P<minor>\d+))?(?:\.(?P<patch>\d+))?"
    r"(?:[-+][0-9A-Za-z.-]+)?\s*$"
)
UPPER_BOUND_OPERATORS = frozenset({"<", "<=", ">"})


class ReleaseContractError(ValueError):
    """A generated package manifest violates the unified release contract."""

    @staticmethod
    def _unreadable_message(
        path: Path, error: OSError | tomllib.TOMLDecodeError
    ) -> str:
        """Return an error message for an unreadable generated manifest."""
        return f"cannot read {path}: {error}"

    @staticmethod
    def _missing_table_message(description: str) -> str:
        """Return an error message for a required missing TOML table."""
        return f"root Cargo.toml has no {description}"

    @staticmethod
    def _invalid_publish_order_message() -> str:
        """Return an error message for an invalid Lading release train."""
        return "lading.toml publish order must be a list of package names"

    @staticmethod
    def _non_string_version_message(subject: str) -> str:
        """Return an error message for a version requirement with the wrong type."""
        return f"{subject} must be a string version requirement"

    @staticmethod
    def _unknown_lower_bound_message(subject: str, value: str) -> str:
        """Return an error message for a version requirement without a bound."""
        return f"cannot determine the version lower bound for {subject}: {value}"

    @staticmethod
    def _invalid_dependency_message(dependency: str) -> str:
        """Return an error message for an invalid dependency declaration."""
        return f"{dependency} has an invalid dependency specification"

    @staticmethod
    def _violations_message(errors: list[str]) -> str:
        """Return one message that reports every release-contract violation."""
        return "\n".join(errors)


def _load_toml(path: Path) -> dict[str, object]:
    """Load one TOML document from *path*."""
    try:
        with path.open("rb") as stream:
            return tomllib.load(stream)
    except (OSError, tomllib.TOMLDecodeError) as error:
        raise ReleaseContractError(
            ReleaseContractError._unreadable_message(path, error)
        ) from error


def workspace_release_version(repository: Path) -> tuple[int, int, int]:
    """Return the root workspace's numeric release version.

    Returns
    -------
    tuple[int, int, int]
        The major, minor, and patch components of the workspace release.

    Raises
    ------
    ReleaseContractError
        The root manifest has no valid workspace package version.
    """
    manifest = _load_toml(repository / "Cargo.toml")
    workspace = manifest.get("workspace")
    if not isinstance(workspace, dict):
        raise ReleaseContractError(
            ReleaseContractError._missing_table_message("workspace table")
        )
    package = workspace.get("package")
    if not isinstance(package, dict):
        raise ReleaseContractError(
            ReleaseContractError._missing_table_message("workspace package table")
        )
    version = package.get("version")
    return parse_version(version, "the workspace release version")


def publish_order(repository: Path) -> tuple[str, ...]:
    """Return the release train declared by ``lading.toml``.

    Returns
    -------
    tuple[str, ...]
        The package names in dependency-first publication order.

    Raises
    ------
    ReleaseContractError
        The Lading configuration has no valid publish order.
    """
    manifest = _load_toml(repository / "lading.toml")
    publish = manifest.get("publish")
    if not isinstance(publish, dict):
        raise ReleaseContractError(
            ReleaseContractError._invalid_publish_order_message()
        )
    order = publish.get("order")
    if not isinstance(order, list) or not all(isinstance(name, str) for name in order):
        raise ReleaseContractError(
            ReleaseContractError._invalid_publish_order_message()
        )
    return tuple(order)


def _version_clause_lower_bound(
    clause: str, subject: str, value: str
) -> tuple[int, int, int] | None:
    """Return one clause's lower bound, or ``None`` for an upper bound."""
    match = VERSION_PATTERN.fullmatch(clause)
    if match is None:
        raise ReleaseContractError(
            ReleaseContractError._unknown_lower_bound_message(subject, value)
        )
    if match.group("operator") in UPPER_BOUND_OPERATORS:
        return None
    return (
        int(match.group("major")),
        int(match.group("minor") or "0"),
        int(match.group("patch") or "0"),
    )


def parse_version(value: object, subject: str) -> tuple[int, int, int]:
    """Return the numeric lower bound from Cargo version *value* for *subject*.

    Returns
    -------
    tuple[int, int, int]
        The major, minor, and patch components of the requirement's lower bound.

    Raises
    ------
    ReleaseContractError
        The requirement is malformed, has no version, or has only upper bounds.
    """
    if not isinstance(value, str):
        raise ReleaseContractError(
            ReleaseContractError._non_string_version_message(subject)
        )
    bounds = [
        bound
        for clause in value.split(",")
        if (bound := _version_clause_lower_bound(clause, subject, value)) is not None
    ]
    if not bounds:
        raise ReleaseContractError(
            ReleaseContractError._unknown_lower_bound_message(subject, value)
        )
    return max(bounds)


def _dependency_tables(
    manifest: cabc.Mapping[str, object],
) -> cabc.Iterator[cabc.Mapping[str, object]]:
    """Yield all standard and target-qualified dependency tables in *manifest*."""
    for name, value in manifest.items():
        if name.endswith("dependencies") and isinstance(value, dict):
            yield value
    targets = manifest.get("target")
    if not isinstance(targets, dict):
        return
    for target in targets.values():
        if isinstance(target, dict):
            yield from _dependency_tables(target)


def dependency_version(specification: object, dependency: str) -> tuple[int, int, int]:
    """Return the declared version lower bound for internal *dependency*.

    Returns
    -------
    tuple[int, int, int]
        The major, minor, and patch components of the dependency lower bound.

    Raises
    ------
    ReleaseContractError
        The declaration is invalid or has no usable version lower bound.
    """
    match specification:
        case str():
            return parse_version(specification, dependency)
        case dict():
            return parse_version(specification.get("version"), dependency)
        case _:
            raise ReleaseContractError(
                ReleaseContractError._invalid_dependency_message(dependency)
            )


def _manifest_dependency_errors(
    manifest_path: Path,
    manifest: cabc.Mapping[str, object],
    packages: tuple[str, ...],
    release_version: tuple[int, int, int],
) -> list[str]:
    """Return internal dependency lower-bound violations in one manifest."""
    errors: list[str] = []
    for table in _dependency_tables(manifest):
        for dependency, specification in table.items():
            if dependency not in packages:
                continue
            try:
                lower_bound = dependency_version(specification, dependency)
            except ReleaseContractError as error:
                errors.append(f"{manifest_path}: {error}")
                continue
            if lower_bound < release_version:
                errors.append(
                    f"{manifest_path}: {dependency} lower bound "
                    f"{'.'.join(map(str, lower_bound))} is older than "
                    f"workspace release {'.'.join(map(str, release_version))}"
                )
    return errors


def validate_staged_package_manifests(repository: Path, staged_dir: Path) -> None:
    """Raise when staged release manifests use stale internal dependency bounds.

    The configured Lading order is the source of truth for the publishable
    release train. Each generated manifest must exist, and every dependency on
    another package in that train must require at least the root workspace
    release version.

    Raises
    ------
    ReleaseContractError
        A staged manifest is missing or declares a stale internal lower bound.
    """
    release_version = workspace_release_version(repository)
    packages = publish_order(repository)
    errors: list[str] = []
    for package in packages:
        manifest_path = (
            staged_dir
            / f"{package}-{'.'.join(map(str, release_version))}"
            / "Cargo.toml"
        )
        try:
            manifest = _load_toml(manifest_path)
        except ReleaseContractError as error:
            errors.append(str(error))
            continue
        errors.extend(
            _manifest_dependency_errors(
                manifest_path, manifest, packages, release_version
            )
        )
    if errors:
        raise ReleaseContractError(ReleaseContractError._violations_message(errors))


def _parse_args(argv: list[str] | None = None) -> argparse.Namespace:
    """Parse command-line arguments for the staged package manifest gate."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repository", type=Path, default=Path.cwd())
    parser.add_argument("--staged-dir", type=Path, required=True)
    return parser.parse_args(argv)


def _main(argv: list[str] | None = None) -> int:
    """Validate staged package manifests and return the process status."""
    arguments = _parse_args(argv)
    try:
        validate_staged_package_manifests(arguments.repository, arguments.staged_dir)
    except ReleaseContractError as error:
        print(error, file=sys.stderr)
        return 1
    print("staged package manifests require the current workspace release version")
    return 0


if __name__ == "__main__":
    raise SystemExit(_main())
