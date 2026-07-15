#!/usr/bin/env -S uv run python
# /// script
# requires-python = ">=3.13"
# dependencies = ["pathspec==1.1.1"]
# ///
"""Enforce exact phrase corrections alongside the Typos scanner.

The checker loads shared and repository-local phrase policy, then scans eligible
tracked UTF-8 text. It complements the Markdown-only Typos pass by catching
prohibited forms in human-facing source comments and tests.

Run it from the repository root with
``uv run scripts/typos_rollout_check.py --repository .``.
"""

from __future__ import annotations

import argparse
from collections.abc import Iterator, Sequence
from dataclasses import dataclass
from pathlib import Path
import re
import subprocess
import tomllib

from pathspec import GitIgnoreSpec

POLICY_PATHS = frozenset(
    {
        Path(".typos-oxendict-base.toml"),
        Path("typos.local.toml"),
        Path("typos.toml"),
    }
)


@dataclass(frozen=True)
class PhraseFinding:
    """Describe one prohibited phrase in tracked text.

    Attributes
    ----------
    path
        Repository-relative path containing the phrase.
    line
        One-based source line number.
    column
        One-based source column number.
    phrase
        Source phrase preserving its original case.
    correction
        Replacement prescribed by the spelling policy.
    """

    path: Path
    line: int
    column: int
    phrase: str
    correction: str


@dataclass(frozen=True)
class PhrasePolicy:
    """Hold the effective policy needed by the consumer phrase scanner.

    Attributes
    ----------
    phrase_corrections
        Ordered prohibited phrase and replacement pairs.
    ignore_patterns
        Regular expressions that mask ignored spans.
    excluded_files
        Gitignore-style patterns for excluded paths.
    """

    phrase_corrections: tuple[tuple[str, str], ...]
    ignore_patterns: tuple[str, ...]
    excluded_files: tuple[str, ...]


def _document(path: Path) -> dict[str, object]:
    """Load one TOML policy document."""
    with path.open("rb") as stream:
        return tomllib.load(stream)


def _table(document: dict[str, object], name: str) -> dict[str, object]:
    """Return a TOML table or an empty table when it is absent."""
    value = document.get(name, {})
    return value if isinstance(value, dict) else {}


def _strings(table: dict[str, object], key: str) -> tuple[str, ...]:
    """Return string entries from an effective Typos list."""
    value = table.get(key, [])
    if not isinstance(value, list):
        return ()
    return tuple(item for item in value if isinstance(item, str))


def _phrases(document: dict[str, object]) -> dict[str, str]:
    """Return phrase corrections from one shared-policy document."""
    corrections = _table(_table(document, "phrases"), "corrections")
    return {
        phrase: correction
        for phrase, correction in corrections.items()
        if isinstance(correction, str)
    }


def load_policy(repository: Path) -> PhrasePolicy:
    """Load generated scan policy and shared phrase corrections.

    Parameters
    ----------
    repository
        Git repository containing the generated, shared, and local policies.

    Returns
    -------
    PhrasePolicy
        Effective phrase corrections, ignored spans, and path exclusions.

    Raises
    ------
    FileNotFoundError
        A required generated or shared policy file is missing.
    OSError
        A policy file cannot be read.
    tomllib.TOMLDecodeError
        A policy file contains invalid TOML.
    """
    generated = _document(repository / "typos.toml")
    shared_cache = repository / ".typos-oxendict-base.toml"
    if not shared_cache.is_file():
        message = (
            f"{shared_cache} is missing; regenerate the spelling configuration "
            "as documented in docs/developers-guide.md"
        )
        raise FileNotFoundError(message)
    phrases = _phrases(_document(shared_cache))
    local_overlay = repository / "typos.local.toml"
    if local_overlay.exists():
        phrases.update(_phrases(_document(local_overlay)))
    return PhrasePolicy(
        phrase_corrections=tuple(sorted(phrases.items())),
        ignore_patterns=_strings(_table(generated, "default"), "extend-ignore-re"),
        excluded_files=_strings(_table(generated, "files"), "extend-exclude"),
    )


def _tracked(repository: Path) -> tuple[Path, ...]:
    """Return tracked paths in deterministic order."""
    raw = subprocess.run(
        ["git", "-C", str(repository), "ls-files", "-z"],
        check=True,
        capture_output=True,
        text=True,
    ).stdout
    return tuple(Path(item) for item in sorted(filter(None, raw.split("\0"))))


def _exclusion_spec(policy: PhrasePolicy) -> GitIgnoreSpec:
    """Build the gitignore-style matcher used by Typos exclusions."""
    return GitIgnoreSpec.from_lines(policy.excluded_files)


def _excluded(path: Path, spec: GitIgnoreSpec) -> bool:
    """Return whether effective Typos policy excludes a tracked path."""
    return spec.match_file(path.as_posix())


def _masked(text: str, patterns: tuple[str, ...]) -> str:
    """Blank ignored spans while preserving line and column positions."""

    def blank(match: re.Match[str]) -> str:
        return "".join(
            "\n" if character == "\n" else " " for character in match.group()
        )

    for pattern in patterns:
        text = re.sub(pattern, blank, text)
    return text


def _phrase_findings(
    relative: Path,
    text: str,
    masked: str,
    phrase_corrections: tuple[tuple[str, str], ...],
) -> Iterator[PhraseFinding]:
    """Yield exact phrase findings from one masked tracked file."""
    for phrase, correction in phrase_corrections:
        for match in re.finditer(
            rf"(?<![\w-]){re.escape(phrase)}(?![\w-])", masked, re.IGNORECASE
        ):
            previous = masked.rfind("\n", 0, match.start())
            yield PhraseFinding(
                relative,
                masked.count("\n", 0, match.start()) + 1,
                match.start() - previous,
                text[match.start() : match.end()],
                correction,
            )


def check_phrase_corrections(
    repository: Path, policy: PhrasePolicy
) -> tuple[PhraseFinding, ...]:
    """Find prohibited exact phrases in tracked UTF-8 text.

    Parameters
    ----------
    repository
        Git repository whose tracked files should be scanned.
    policy
        Effective corrections, ignored spans, and path exclusions.

    Returns
    -------
    tuple[PhraseFinding, ...]
        Findings in deterministic path, phrase, and source order.

    Raises
    ------
    subprocess.CalledProcessError
        Git cannot enumerate the repository's tracked files.
    """
    found = []
    exclusion_spec = _exclusion_spec(policy)
    for relative in _tracked(repository):
        if relative in POLICY_PATHS or _excluded(relative, exclusion_spec):
            continue
        try:
            text = (repository / relative).read_text(encoding="utf-8")
        except (OSError, UnicodeDecodeError):
            continue
        masked = _masked(text, policy.ignore_patterns)
        found.extend(
            _phrase_findings(relative, text, masked, policy.phrase_corrections)
        )
    return tuple(found)


def main(argv: Sequence[str] | None = None) -> int:
    """Report prohibited phrases and return the spelling-gate status.

    Parameters
    ----------
    argv
        Arguments excluding the executable name, or ``None`` to read the
        process command line.

    Returns
    -------
    int
        Two when prohibited phrases are found, otherwise zero.

    Raises
    ------
    FileNotFoundError
        A required generated or shared policy file is missing.
    OSError
        A policy file cannot be read.
    subprocess.CalledProcessError
        Git cannot enumerate the repository's tracked files.
    tomllib.TOMLDecodeError
        A policy file contains invalid TOML.
    """
    parser = argparse.ArgumentParser()
    parser.add_argument("--repository", type=Path, default=Path.cwd())
    repository = parser.parse_args(argv).repository
    findings = check_phrase_corrections(repository, load_policy(repository))
    for item in findings:
        print(
            f"{item.path}:{item.line}:{item.column}: {item.phrase} -> {item.correction}"
        )
    return 2 if findings else 0


if __name__ == "__main__":
    raise SystemExit(main())
