"""The publish report's verification script, executed.

Asserting that the script's text contains `::warning` says nothing about
which branch reaches it, nor whether the job survives. These run the
script the workflow declares, against a report that is missing, empty,
malformed and valid in turn, and read what it prints.

The script is written to a file and run as `bash <file>` rather than
passed to `bash -c`. Bash 3.2 exec-replaces itself with the last
external command of a `-c` string, so a harness using that form measures
something the runner never does; runners execute fragments from a file.

Run via ``make test-workflow-contracts``.
"""

import typing as typ

from publish_report_support import run_verification

if typ.TYPE_CHECKING:
    from pathlib import Path


class TestTheVerificationScriptRuns:
    """The script itself, executed, rather than read for substrings.

    Asserting that the text contains ``::warning`` says nothing about
    which branch reaches it or whether the job survives. These run the
    script the workflow declares, against a report that is missing,
    empty, malformed and valid in turn.
    """

    def test_a_missing_report_warns_and_leaves_the_job_alive(
        self, build_test_job: dict[str, typ.Any], tmp_path: Path
    ) -> None:
        """The common case: the publish step failed before lading ran.

        The warning has to name the path and offer a cause, because the
        artefact list looks identical whether the report was never
        written or was written and never read.
        """
        result = run_verification(build_test_job, tmp_path, None)

        assert result.returncode == 0, result.stderr
        # Three parts of one sentence, checked separately so a failure
        # names which part is missing: the annotation GitHub renders, the
        # cause a maintainer acts on, and the path they look for. A
        # snapshot would fix the whole wording, which changes for reasons
        # this contract has no opinion about.
        expected = (
            "::warning title=publish-statistics::",
            "wrote no compiler-cache report",
            "sccache-publish.json",
        )
        missing = [fragment for fragment in expected if fragment not in result.stdout]
        assert not missing, (
            f"the warning must carry the annotation, the cause and the path; "
            f"missing {missing} from {result.stdout!r}"
        )

    def test_an_empty_report_is_distinguished_from_a_missing_one(
        self, build_test_job: dict[str, typ.Any], tmp_path: Path
    ) -> None:
        """Lading created the file and wrote nothing to it.

        A different fault from never running, and the message says so:
        collapsing the two would send the reader looking for a publish
        failure that did not happen.
        """
        result = run_verification(build_test_job, tmp_path, b"")

        assert result.returncode == 0, result.stderr
        assert "is empty" in result.stdout, result.stdout
        assert "wrote no compiler-cache report" not in result.stdout, result.stdout

    def test_a_malformed_report_is_reported_as_unreadable(
        self, build_test_job: dict[str, typ.Any], tmp_path: Path
    ) -> None:
        """A truncated write leaves a non-empty file that is not JSON.

        Testing only for existence would pass this, which is why the
        script parses rather than stats.
        """
        result = run_verification(build_test_job, tmp_path, b'{"requests": ')

        assert result.returncode == 0, result.stderr
        assert "not valid JSON" in result.stdout, result.stdout

    def test_a_valid_report_is_printed_rather_than_warned_about(
        self, build_test_job: dict[str, typ.Any], tmp_path: Path
    ) -> None:
        """The successful case, which must produce no warning at all.

        A script that warned unconditionally would pass all three cases
        above and tell a maintainer nothing.
        """
        report = b'{"stats": {"compile_requests": 42, "cache_hits": 7}}'
        result = run_verification(build_test_job, tmp_path, report)

        assert result.returncode == 0, result.stderr
        assert "::warning" not in result.stdout, result.stdout
        assert "Publish-step compiler-cache report:" in result.stdout, result.stdout
        assert "compile_requests" in result.stdout, result.stdout
