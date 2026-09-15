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

import json
import os
import sys
from pathlib import Path

#: The environment variable naming the report. The workflow sets it to the
#: same path it gives lading, so the reader and the writer cannot drift.
STATS_PATH_VARIABLE = "STATS_PATH"

#: The annotation title GitHub groups these warnings under.
WARNING_TITLE = "publish-statistics"


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


def describe_report(stats_path: Path) -> str | None:
    """Return the report's text, or ``None`` after warning about its absence.

    Parameters
    ----------
    stats_path : Path
        Where the publish step was told to write the report.

    Returns
    -------
    str or None
        The report's text when it is present and parsable, otherwise
        ``None``, with the reason already announced.

    Examples
    --------
    >>> describe_report(Path("no-such-report.json"))  # doctest: +ELLIPSIS
    ::warning title=publish-statistics::lading wrote no compiler-cache...
    """
    if not stats_path.is_file():
        warn(
            f"lading wrote no compiler-cache report to {stats_path}; either the "
            f"publish step failed before lading ran, or the resolved lading "
            f"predates LADING_SCCACHE_STATS_JSON. This run's publish cost is "
            f"unattributable."
        )
        return None
    try:
        text = stats_path.read_text(encoding="utf-8")
    except (OSError, UnicodeDecodeError) as error:
        # The file passed is_file() a moment ago, so reaching here means the
        # report is binary, truncated to non-UTF-8 bytes, or was removed or
        # made unreadable between the two calls. None of that is a build
        # failure, and the guarantee this module makes is that none of it
        # fails the lane either.
        warn(f"{stats_path} could not be read: {error}.")
        return None
    if not text.strip():
        warn(f"{stats_path} is empty; lading created it but wrote no report.")
        return None
    try:
        json.loads(text)
    except ValueError:
        warn(
            f"{stats_path} is not valid JSON, so the publish step's "
            f"compiler-cache report cannot be read."
        )
        return None
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
    text = describe_report(Path(raw_path))
    if text is None:
        return 0
    print("Publish-step compiler-cache report:")
    print(text, end="" if text.endswith("\n") else "\n")
    return 0


if __name__ == "__main__":
    sys.exit(main())
