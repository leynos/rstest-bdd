//! The report lines this crate files, against a reader in another language.
//!
//! The format is a contract with `scripts/prepared_fixture_step.py`, which
//! parses it with a regular expression and hands the results to
//! `prepared_fixture_judgements.expect_pass_without_rebuild`. Pinning the
//! writer here means a change to the marker, the field names or the count is
//! caught by a Rust test, rather than only by a cold end-to-end workflow run
//! that nobody has recently afforded.
//!
//! The lines are produced by the same [`record_to`] the gate and the timed
//! command call, and the count is produced by the same [`count_compiling`] the
//! timed path uses, so what is checked here is the production writer rather
//! than a copy of its format.

use std::path::Path;

use super::nested_build::{ReportSource, record_to};

/// The marker the workflow's reader looks for.
///
/// Transcribed from `prepared_fixture_step.BUILD_MARKER`. The two languages
/// have no common header, so the duplication cannot be removed — only pinned,
/// which is what the cases below do.
const READER_MARKER: &str = "cargo-bdd-nested-build:";

/// Every `(source, count)` a reader following the workflow's grammar finds.
///
/// The grammar is the reader's `REPORT_LINE` expression applied to one line:
/// the marker, then `source=<word>`, then `compiling=<digits>`, separated by
/// whitespace. Written as token matching rather than as a regular expression so
/// this crate needs no pattern dependency for one assertion; a line either has
/// exactly the four tokens the reader expects or it contributes nothing, which
/// is the discrimination that matters.
fn as_the_reader_reads(report: &Path) -> Vec<(String, usize)> {
    let text = std::fs::read_to_string(report).expect("the report is readable");
    text.lines()
        .filter_map(|line| {
            let mut tokens = line.split_whitespace();
            if tokens.next() != Some(READER_MARKER) {
                return None;
            }
            let source = tokens.next()?.strip_prefix("source=")?;
            let count = tokens.next()?.strip_prefix("compiling=")?;
            Some((source.to_owned(), count.parse().ok()?))
        })
        .collect()
}

#[test]
fn the_gate_line_is_one_the_workflow_reader_parses() {
    // The gate's zero is the line that proves a run reached the gate, and the
    // reader requires exactly one of them per run. A writer whose spelling
    // drifted from the reader's would leave the workflow unable to see that the
    // run had gated at all.
    let scratch = tempfile::tempdir().expect("a scratch directory");
    let report = scratch.path().join("report.txt");

    record_to(&report, ReportSource::Gate, 0);

    assert_eq!(
        as_the_reader_reads(&report),
        vec![("gate".to_owned(), 0)],
        "the gate's line must parse as one zero from source `gate`"
    );
    assert_eq!(
        std::fs::read_to_string(&report).expect("the report is readable"),
        "cargo-bdd-nested-build: source=gate compiling=0\n",
        "the reader's expression is whitespace-tolerant; the writer's spelling is not"
    );
}

#[test]
fn every_source_files_its_own_name() {
    // The reader attributes a line by its source, so the labels are what
    // separate "the gate admitted a stamp" from "the timed command compiled
    // this much". Two lines sharing a label would make a run's evidence
    // ambiguous in exactly the way the judgement now rejects.
    let scratch = tempfile::tempdir().expect("a scratch directory");
    let report = scratch.path().join("report.txt");

    record_to(&report, ReportSource::Preparation, 51);
    record_to(&report, ReportSource::Gate, 0);
    record_to(&report, ReportSource::Timed, 0);

    assert_eq!(
        as_the_reader_reads(&report),
        vec![
            ("preparation".to_owned(), 51),
            ("gate".to_owned(), 0),
            ("timed".to_owned(), 0),
        ],
        "each source keeps its own label and the lines stay in the order written"
    );
}
