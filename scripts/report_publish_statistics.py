#!/usr/bin/env python3
"""Report the publish step's compiler-cache statistics, or say why there are none.

``lading publish`` writes a JSON report to the path named by
``LADING_SCCACHE_STATS_JSON``; the workflow then runs this to read it before
the upload collects it. An absent report and a report nobody opened look
identical in an artefact list, so each way of having nothing gets its own
message: never written, written empty, written and unreadable, or
written and unparsable.

Nothing here fails the job. The report is evidence about a build rather than
the build, and a publish that failed before lading ran has already failed on
its own account. Every outcome exits zero; the failures are announced as
GitHub workflow warnings.

Reading and announcing are separate. :func:`read_report` decides and returns
its outcome, and :func:`main` is the only place that writes an annotation, so
a reason can be asserted as a value rather than scraped from captured output.

The path is read from the environment rather than taken as an argument so the
same invocation works on both runner families: on Windows it is a backslashed
path that a shell fragment would have to quote correctly, and here it is only
ever a string handed to :class:`pathlib.Path`.

Examples
--------
```console
$ STATS_PATH=/tmp/sccache-publish.json python3 scripts/report_publish_statistics.py
Publish-step compiler-cache report:
{"delta": {"hits": 17, "misses": 15}}
```
"""

import dataclasses
import json
import os
import sys
from pathlib import Path

#: The environment variable naming the report. The workflow sets it to the
#: same path it gives lading, so the reader and the writer cannot drift.
STATS_PATH_VARIABLE = "STATS_PATH"

#: The annotation title GitHub groups these warnings under.
WARNING_TITLE = "publish-statistics"


@dataclasses.dataclass(frozen=True, slots=True)
class Unavailable:
    """Why the report cannot be had, in the words its warning will carry.

    A distinct type rather than ``None`` because the four ways of having
    nothing differ only in their reason, and a caller that loses the reason
    has lost the whole diagnostic.

    Attributes
    ----------
    reason : str
        What was expected, and what was found instead.
    """

    reason: str


def warn(message: str) -> None:
    """Announce a missing or unreadable report as a workflow warning.

    Parameters
    ----------
    message : str
        What was expected, and what was found instead.

    Examples
    --------
    >>> warn("nothing to read")
    ::warning title=publish-statistics::nothing to read
    """
    print(f"::warning title={WARNING_TITLE}::{message}")


def read_report(stats_path: Path) -> str | Unavailable:
    """Return the report's text, or the reason there is none to return.

    Parameters
    ----------
    stats_path : Path
        Where the publish step was told to write the report.

    Returns
    -------
    str or Unavailable
        The report's text when it is present, decodable, non-empty and
        parsable; otherwise the reason it is not, unannounced.

    Examples
    --------
    >>> read_report(Path("no-such-report.json")).reason  # doctest: +ELLIPSIS
    "lading wrote no compiler-cache report to no-such-report.json; ..."
    """
    if not stats_path.is_file():
        return Unavailable(
            f"lading wrote no compiler-cache report to {stats_path}; either the "
            f"publish step failed before lading ran, or the resolved lading "
            f"predates LADING_SCCACHE_STATS_JSON. This run's publish cost is "
            f"unattributable."
        )
    try:
        text = stats_path.read_text(encoding="utf-8")
    except (OSError, UnicodeDecodeError) as error:
        # The file passed is_file() a moment ago, so reaching here means the
        # report is binary, truncated to non-UTF-8 bytes, or was removed or
        # made unreadable between the two calls. None of that is a build
        # failure, and the guarantee this module makes is that none of it
        # fails the lane either.
        return Unavailable(f"{stats_path} could not be read: {error}.")
    if not text.strip():
        return Unavailable(
            f"{stats_path} is empty; lading created it but wrote no report."
        )
    try:
        json.loads(text)
    except ValueError:
        return Unavailable(
            f"{stats_path} is not valid JSON, so the publish step's "
            f"compiler-cache report cannot be read."
        )
    return text


def main() -> int:
    """Print the report named by the environment, or warn about its absence.

    Returns
    -------
    int
        Always zero: this step reports on a build rather than being one.
    """
    raw_path = os.environ.get(STATS_PATH_VARIABLE, "")
    if not raw_path:
        warn(
            f"{STATS_PATH_VARIABLE} is unset, so the publish step's "
            f"compiler-cache report cannot be located."
        )
        return 0
    outcome = read_report(Path(raw_path))
    if isinstance(outcome, Unavailable):
        warn(outcome.reason)
        return 0
    print("Publish-step compiler-cache report:")
    print(outcome, end="" if outcome.endswith("\n") else "\n")
    return 0


if __name__ == "__main__":
    sys.exit(main())
