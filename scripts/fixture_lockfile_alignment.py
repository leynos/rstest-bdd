#!/usr/bin/env python3
"""Check that each fixture resolves to the workspace's own dependency graph.

A standalone fixture commits its own lockfile, so Cargo resolves it
independently of the workspace. That independence is what makes the fixture
hermetic, and it is also what makes it expensive: every crate the fixture
resolves to a version or source the workspace did not pick is a compiled unit
the workspace build cannot lend it, built a second time inside the nested run.
The nested runs already serialize behind one nextest test group, so a fixture
graph that diverges from the workspace is paid for on the critical path of
every cold CI lane.

Byte-identical lockfiles are not the requirement and would be the wrong one:
the fixture's own package appears only in its lockfile, and a fixture may
legitimately step outside the workspace graph. The requirement is that each
``(name, version, source)`` the fixture resolves is one the workspace also
resolved. A fixture that passes adds only its own crate to the build; a fixture
that fails adds the difference, and the failure names exactly which packages
those are.

The workspace graph is read from the root lockfile directly rather than from
``cargo metadata``: the check runs in the same gate as the lockfile validation,
must not need a network, and the root lockfile *is* the resolution the workspace
build uses. Each fixture's lockfile is read the same way, so both sides of the
comparison are the same kind of document.

Usage: ``python3 scripts/fixture_lockfile_alignment.py``.

Exit codes: 0 when every discovered fixture is a subset of the workspace graph;
1 when one is not, or when a lockfile cannot be read.

The comparison is a query and the reports are presentation, so
:func:`alignment_divergences` answers with ``(manifest, packages)`` pairs and
writes nothing, :func:`~fixture_lockfile_reporting.alignment_failure` renders
one pair, and :func:`check_alignment` owns the stream output. A caller that
wants the structure never has to capture a stream to get it.
"""

import sys
from itertools import starmap
from pathlib import Path

from fixture_lockfile_discovery import discover_fixture_manifests
from fixture_lockfile_packages import (
    LockfileUnreadableError,
    PackageKey,
    own_package,
    read_lockfile_packages,
)

# The report wording lives with the rest of the gate's wording, in the
# reporting module. ``alignment_failure`` is imported to render the pairs
# :func:`alignment_divergences` answers with, and both it and ``format_package``
# stay in this module's ``__all__`` so a caller that already imports them from
# here keeps working. ``print_failures`` is the shared printer
# :func:`check_alignment` emits through.
from fixture_lockfile_reporting import (
    FixtureLockfileError,
    alignment_failure,
    format_package,
    print_failures,
)

#: Fixtures excluded from the alignment requirement, by root-relative directory.
#:
#: Both are excluded for the same structural reason: their builds cannot reuse
#: the workspace's compiled units no matter what they resolve, so aligning them
#: would buy wall-clock time that the nested lane never spends.
#:
#: * ``ui_lints`` builds through ``CARGO_TARGET_DIR=target/tests/ui_lints_clippy``
#:   (``trybuild_macros.rs``) so lint runs cannot pollute the shared target
#:   directory. Its artefacts are isolated by construction.
#: * ``published-gpui-0-2-2`` exists to resolve ``gpui`` from crates.io rather
#:   than the workspace's vendored path dependency, which is the opposite of the
#:   alignment requirement.
EXEMPT_FIXTURES = frozenset({
    "crates/rstest-bdd/tests/ui_lints",
    "tests/fixtures/published-gpui-0-2-2",
})

#: Fixtures a refresh must resolve from their own lockfile, by root-relative
#: directory.
#:
#: Seeding a lockfile from the root ``Cargo.lock`` is what keeps a refresh
#: aligned, and it is exactly wrong for a fixture that resolves outside the
#: workspace on purpose: ``published-gpui-0-2-2`` exists to prove the published
#: ``gpui`` 0.2.2 resolves from crates.io rather than the vendored path
#: dependency. Seeding it replaced its 568 crates.io-only packages with the
#: workspace's resolution, so the fixture silently stopped testing what it was
#: written to test. Its ``make update-published-gpui-0-2-2-lock`` target owns
#: that lockfile instead.
SEED_EXCLUDED_FIXTURES = frozenset({"tests/fixtures/published-gpui-0-2-2"})

# Re-exported: the alignment comparison, its reading, and the report of its
# result are one concept to a caller, so a comparison consumer needs one import
# rather than three.
__all__ = [
    "EXEMPT_FIXTURES",
    "SEED_EXCLUDED_FIXTURES",
    "LockfileUnreadableError",
    "PackageKey",
    "alignment_divergences",
    "alignment_failure",
    "check_alignment",
    "check_alignment_or_failure",
    "divergent_packages",
    "format_package",
    "is_exempt",
    "is_seed_excluded",
    "own_package",
    "read_lockfile_packages",
]


def divergent_packages(
    workspace: set[PackageKey], fixture: set[PackageKey]
) -> list[PackageKey]:
    """Return the fixture packages the workspace graph does not contain.

    Parameters
    ----------
    workspace : set[PackageKey]
        Every package the workspace lockfile resolves.
    fixture : set[PackageKey]
        Every package the fixture lockfile resolves.

    Returns
    -------
    list[PackageKey]
        The fixture's packages absent from the workspace graph, sorted so a
        failure report is stable between runs.

    Examples
    --------
    >>> divergent_packages({("a", "1.0.0", None)}, {("a", "1.0.0", None)})
    []
    >>> divergent_packages({("a", "1.0.0", None)}, {("a", "2.0.0", None)})
    [('a', '2.0.0', None)]
    """
    return sorted(fixture - workspace)


def _relative_fixture_dir(manifest: Path, root: Path) -> str | None:
    """Return *manifest*'s fixture directory relative to *root*, or None."""
    try:
        return manifest.parent.relative_to(root).as_posix()
    except ValueError:
        return None


def is_exempt(manifest: Path, root: Path) -> bool:
    """Return whether *manifest* is exempt from the alignment requirement.

    An exempt fixture is still discovered and validated; exemption only
    removes it from the alignment comparison below.

    Parameters
    ----------
    manifest : Path
        The fixture manifest to classify.
    root : Path
        The repository root that :data:`EXEMPT_FIXTURES` entries are relative
        to.

    Returns
    -------
    bool
        ``True`` when the manifest's own directory is one of the
        :data:`EXEMPT_FIXTURES` entries, and ``False`` for every other
        manifest the comparison covers.
    """
    return _relative_fixture_dir(manifest, root) in EXEMPT_FIXTURES


def is_seed_excluded(manifest: Path, root: Path) -> bool:
    """Return whether *manifest*'s lockfile must keep its own resolution.

    A refresh seeds every other fixture from the workspace lock; for these it
    would overwrite the very resolution the fixture exists to prove, so they
    are left to their own Makefile target.

    Parameters
    ----------
    manifest : Path
        The fixture manifest to classify.
    root : Path
        The repository root that :data:`SEED_EXCLUDED_FIXTURES` entries are
        relative to.

    Returns
    -------
    bool
        ``True`` when the manifest's directory is one of the
        :data:`SEED_EXCLUDED_FIXTURES` entries.
    """
    return _relative_fixture_dir(manifest, root) in SEED_EXCLUDED_FIXTURES


def alignment_divergences(root: Path) -> list[tuple[Path, list[PackageKey]]]:
    """Return each fixture's divergent packages, keyed by its manifest.

    The comparison is a query: it reads both lockfiles and reports what
    differs, leaving the rendering of that result to
    :func:`~fixture_lockfile_reporting.alignment_failure` at the presentation
    boundary.

    Parameters
    ----------
    root : Path
        The repository root.

    Returns
    -------
    list[tuple[Path, list[PackageKey]]]
        One ``(manifest, divergent packages)`` pair per fixture that resolves
        outside the workspace graph, in discovery order. Empty when every
        fixture is aligned.
    """
    workspace = read_lockfile_packages(root / "Cargo.lock")
    divergences = []
    for manifest in discover_fixture_manifests(root):
        if is_exempt(manifest, root):
            continue
        fixture = read_lockfile_packages(manifest.parent / "Cargo.lock")
        own = own_package(manifest)
        if own is not None:
            fixture -= {own}
        packages = divergent_packages(workspace, fixture)
        if packages:
            divergences.append((manifest, packages))
    return divergences


def check_alignment(root: Path) -> int:
    """Report every fixture that resolves outside the workspace graph.

    Composes the comparison with the report renderer, printing each failure to
    standard error through the gate's shared failure printer, so the calling
    gate's own summary stays the last line of its output.

    This is a composition rather than a boundary: a lockfile that cannot be
    read is not a divergence it can describe, so the
    ``LockfileUnreadableError`` its comparison propagates is not caught here
    and no graph is invented for it. A caller at a command boundary wants
    :func:`check_alignment_or_failure`, which turns that into the gate's own
    ``error:`` report and exit code.

    Parameters
    ----------
    root : Path
        The repository root.

    Returns
    -------
    int
        ``0`` when every fixture is aligned, otherwise ``1``.
    """
    failures = list(starmap(alignment_failure, alignment_divergences(root)))
    if failures:
        print_failures(failures)
    return 1 if failures else 0


def check_alignment_or_failure(root: Path) -> int:
    """Report alignment failures, turning an unreadable lockfile into one.

    :func:`check_alignment` is a composition, not a boundary: it lets
    ``LockfileUnreadableError`` escape because a lockfile that cannot be read
    is not a divergence it can describe. A gate that calls it has to supply
    the boundary, or a missing or malformed root lockfile reaches the user as
    an unhandled traceback instead of the documented ``error:`` report and
    exit code. This wrapper is that boundary, so every caller reports the
    condition the same way: the message is a ``FixtureLockfileError``, which
    keeps the gate's own ``error:`` spelling.

    Parameters
    ----------
    root : Path
        The repository root.

    Returns
    -------
    int
        ``0`` when every fixture is aligned, otherwise ``1``.
    """
    try:
        return check_alignment(root)
    except FixtureLockfileError as error:
        print(f"error: {error}", file=sys.stderr)
        return 1


def main() -> int:
    """Run the fixture-alignment gate.

    Returns
    -------
    int
        The exit code: ``0`` when every fixture is aligned, otherwise ``1``.
    """
    root = Path(__file__).resolve().parents[1]
    exit_code = check_alignment_or_failure(root)
    if exit_code == 0:
        print("every fixture lockfile resolves within the workspace graph")
    return exit_code


if __name__ == "__main__":
    sys.exit(main())
