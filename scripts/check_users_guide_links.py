#!/usr/bin/env python3
"""
Validate and regenerate the absolute repository links in ``docs/users-guide.md``.

The users guide is vendored into consumer projects, so its cross-references to
other documents in this repository use absolute GitHub URLs rather than
relative paths. Where those links point is recorded once, in
``scripts/users_guide_links.py``, and the guide's reference definitions are
generated from it: ``make update-users-guide-links`` rewrites the block, and
``make lint`` -- which runs this script without ``--fix`` -- fails while the
committed block disagrees with what that command would write. A branch rename,
a repository move, or a documentation relocation is therefore one constant and
one command rather than an edit to every definition.

Definitions that are neither canonical nor recognizable but point into this
repository are reported rather than skipped, so a target document that has gone
missing is still caught; links to other projects (for example docs.rs) are
ignored.

Proportionality (issue #540): the validator was judged proportionate and
retained (see ADR-014). The users guide is vendored into consumer projects, so
a broken absolute link ships silently to downstream users and is otherwise
caught only by manual review; the checker turns that drift into a deterministic
lint failure. Scope deliberately stays limited to the guide's repository
reference links rather than all documentation cross-references.

Usage
-----
python3 scripts/check_users_guide_links.py [--root PATH] [--fix]

``--root`` overrides the repository root (the directory containing ``docs/``);
it defaults to the parent of this script's directory. The override lets tests
point the script at a temporary tree, and lets another checkout be validated or
rewritten locally. ``--fix`` rewrites the guide in place and then reports
whatever is still invalid.

Exit codes
----------
0
    Every repository link uses the canonical base URL, resolves to an existing
    document, and any fragment matches a heading anchor; or ``--fix`` left the
    guide in that state.
1
    Violations found, or the guide itself could not be read.
"""

import argparse
import sys
import typing as typ
from pathlib import Path

from markdown_references import heading_anchors, reference_definitions
from users_guide_links import (
    BASE_URL,
    DOCS_DIR,
    GUIDE,
    canonical_link,
    count_changed_lines,
    generate_guide,
    is_repository_link,
)

if typ.TYPE_CHECKING:
    import collections.abc as cabc


def check_repo_link(root: Path, label: str, url: str) -> list[str]:
    """
    Validate one repository reference definition.

    Parameters
    ----------
    root : Path
        The repository root directory.
    label : str
        The reference label, used in violation messages.
    url : str
        The reference URL.

    Returns
    -------
    list[str]
        Human-readable violations; empty when the reference is valid.
    """
    canonical = canonical_link(root, url)
    if canonical is None:
        return [
            (
                f"[{label}] does not name a document under the canonical base URL "
                f"{BASE_URL}: {url}"
            )
        ]
    if canonical != url:
        return [
            (
                f"[{label}] does not use the canonical base URL {BASE_URL}: {url} "
                "(run: make update-users-guide-links)"
            )
        ]

    target, _, fragment = url.removeprefix(BASE_URL).partition("#")
    document = root / DOCS_DIR / target
    try:
        content = document.read_text(encoding="utf-8")
    except FileNotFoundError:
        return [f"[{label}] points at a missing document: {DOCS_DIR}/{target}"]
    except OSError as err:
        return [
            f"[{label}] points at an unreadable document: {DOCS_DIR}/{target} ({err})"
        ]

    if fragment and fragment not in heading_anchors(content):
        return [
            f"[{label}] fragment #{fragment} matches no heading in {DOCS_DIR}/{target}"
        ]

    return []


def check_guide(root: Path) -> list[str]:
    """
    Check every repository reference link in the guide.

    Parameters
    ----------
    root : Path
        The repository root directory.

    Returns
    -------
    list[str]
        Human-readable violations; empty when every link is valid.
    """
    guide = root / GUIDE
    try:
        markdown = guide.read_text(encoding="utf-8")
    except OSError as err:
        return [f"could not read {GUIDE}: {err}"]

    violations: list[str] = []
    repo_links = 0
    for label, url in reference_definitions(markdown):
        if not is_repository_link(root, url):
            continue
        repo_links += 1
        violations.extend(check_repo_link(root, label, url))

    if repo_links == 0:
        violations.append(
            f"no repository reference links found in {GUIDE}; "
            "the reference block may have been removed or reformatted"
        )

    return violations


def run_check(root: Path) -> int:
    """
    Report every violation in the guide's reference block.

    Parameters
    ----------
    root : Path
        The repository root directory.

    Returns
    -------
    int
        ``0`` when the reference block is valid, ``1`` otherwise.
    """
    violations = check_guide(root)
    for violation in violations:
        print(violation, file=sys.stderr)
    return 1 if violations else 0


def run_fix(root: Path) -> int:
    """
    Rewrite the guide's reference block, then report what remains invalid.

    Parameters
    ----------
    root : Path
        The repository root directory.

    Returns
    -------
    int
        ``0`` when the rewritten guide is valid, ``1`` otherwise.
    """
    guide = root / GUIDE
    try:
        markdown = guide.read_text(encoding="utf-8")
    except OSError as err:
        print(f"could not read {GUIDE}: {err}", file=sys.stderr)
        return 1

    generated = generate_guide(root, markdown)
    if generated == markdown:
        print(f"{GUIDE} already matches the generated reference links")
    else:
        guide.write_text(generated, encoding="utf-8")
        changed = count_changed_lines(markdown, generated)
        print(f"rewrote {changed} reference line(s) in {GUIDE}")

    return run_check(root)


def main(argv: cabc.Sequence[str] | None = None) -> int:
    """
    Check or regenerate the guide's repository links.

    Parameters
    ----------
    argv : collections.abc.Sequence[str] | None
        Command-line arguments, excluding the program name. ``None``
        (the default) reads :data:`sys.argv`.

    Returns
    -------
    int
        ``0`` when the guide is valid, ``1`` otherwise.
    """
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--root",
        type=Path,
        default=None,
        help="repository root containing docs/ (defaults to this script's parent)",
    )
    parser.add_argument(
        "--fix",
        action="store_true",
        help="rewrite the guide's reference links instead of only checking them",
    )
    args = parser.parse_args(argv)
    root = args.root if args.root is not None else Path(__file__).resolve().parents[1]
    return run_fix(root) if args.fix else run_check(root)


if __name__ == "__main__":
    sys.exit(main())
