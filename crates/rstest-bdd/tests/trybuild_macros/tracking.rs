//! Tracking-binding dep-info assertion for the staged trybuild fixtures.
//!
//! After the trybuild run has compiled the compile-pass fixtures, this module
//! proves at least one dep-info under the trybuild build tree lists the staged
//! `tracking.feature`. It is the cheap mid-tier signal that catches a future
//! codegen refactor silently dropping the binding, without depending on the
//! expensive nested-cargo regression test (`ExecPlan` Milestone 4).

use std::{
    path::Path as StdPath,
    time::{Duration, SystemTime},
};

use camino::{Utf8Path, Utf8PathBuf};

use crate::staging::trybuild_target_directory;

/// How far before the trybuild run a dep-info's timestamp may fall and still
/// count as freshly written.
///
/// Most filesystems give a file the timestamp of the moment it was written, so
/// this slack is not usually needed. It exists because some give coarse
/// one-second (or worse) timestamps, and a file written moments after the run
/// started can then carry a timestamp slightly before it. Two seconds absorbs
/// that rounding without admitting an artefact from an earlier run, which would
/// be minutes or hours old.
const DEP_INFO_CLOCK_SLACK: Duration = Duration::from_secs(2);

/// Assert that the staged compile-pass fixtures registered the tracking
/// binding: at least one dep-info under the trybuild build tree lists the
/// staged `tracking.feature`.
///
/// `run_started` is captured immediately before the queued fixtures are run.
/// Only dep-info files written at or after it are considered, so the assertion
/// proves this run's code generation reached dep-info rather than merely
/// observing a file an earlier run left behind. `trybuild` cleans the generated
/// package before each case, so every fixture bin is recompiled and its
/// dep-info rewritten on each run.
///
/// This is the cheap mid-tier signal that catches a future codegen refactor
/// silently dropping the binding, without depending on the expensive
/// nested-cargo regression test. The check scans the trybuild artefacts from
/// the run we have already paid for (see the `ExecPlan` Milestone 4 section).
pub(crate) fn assert_trybuild_tracking_registered_in_dep_info(run_started: SystemTime) {
    let target_directory = workspace_target_directory();
    let staged_feature = target_directory.join("tests/trybuild/rstest-bdd/tracking.feature");
    let needle = normalize_staged_feature_path(staged_feature.as_str());
    let threshold = freshness_threshold(run_started);
    let mut listed = 0;
    collect_dep_info_matches(
        target_directory.as_std_path(),
        &needle,
        threshold,
        &mut listed,
    );
    assert!(
        listed > 0,
        "no trybuild dep-info below {target_directory} written during this run lists the staged `{needle}`; the macro-emitted tracking binding is not reaching dep-info, or the fixture that registers it was not recompiled"
    );
}

/// The instant at or after which a dep-info counts as this run's.
///
/// Separated from the assertion so the slack rule can be tested against both
/// sides: a file written inside the slack must be admitted, and one written
/// before it must not. Inlined, the only way to test the rule would be to wait
/// out a real clock interval.
fn freshness_threshold(run_started: SystemTime) -> SystemTime {
    run_started
        .checked_sub(DEP_INFO_CLOCK_SLACK)
        .unwrap_or(run_started)
}

/// Return the workspace target directory containing trybuild's copied source.
///
/// Trybuild keeps its generated fixture crate here. Staging and inspection use
/// the same Cargo-selected root, including a coverage-specific target directory.
fn workspace_target_directory() -> Utf8PathBuf {
    let crate_dir = Utf8Path::new(env!("CARGO_MANIFEST_DIR"));
    crate_dir.parent().and_then(Utf8Path::parent).map_or_else(
        || panic!("workspace root must be two levels above the manifest dir"),
        trybuild_target_directory,
    )
}

/// Normalize a staged feature path for matching against dep-info content.
///
/// This deliberately does not decode dep-info escapes: the staged path is a
/// filesystem path, not a dep-info record.
fn normalize_staged_feature_path(path: &str) -> String {
    normalize_path_separators(path).to_lowercase()
}

/// Fold native separators and separator runs into one portable separator.
///
/// Dep-info and the staged path can render the same UNC path with different
/// separator runs. Keeping the normalization shared makes their comparison
/// independent of that rendering detail.
fn normalize_path_separators(path: &str) -> String {
    let mut normalized = String::with_capacity(path.len());
    let mut previous_was_separator = false;

    for character in path.chars() {
        let is_separator = matches!(character, '/' | '\\');
        if !is_separator || !previous_was_separator {
            normalized.push(if is_separator { '/' } else { character });
        }
        previous_was_separator = is_separator;
    }

    normalized
}

/// Normalize a dep-info's raw text the same way as the needle — `/`
/// separators, folded case — so the Windows CI leg (backslash-separated,
/// drive-letter-cased paths) matches the staged feature path deterministically.
fn normalize_dep_info_content(content: &str) -> String {
    normalize_path_separators(&content.replace("\\ ", " ").replace("\\:", ":")).to_lowercase()
}

/// Recursively count `*.d` files under `dir` that were written at or after
/// `threshold` and whose content contains `needle`.
///
/// Walking the whole trybuild tree keeps the assertion independent of
/// trybuild's per-target directory naming (the project dir hosts a
/// host-triplet subdirectory).
fn collect_dep_info_matches(
    dir: &StdPath,
    needle: &str,
    threshold: SystemTime,
    listed: &mut usize,
) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries {
        let Ok(entry) = entry else {
            continue;
        };
        let path = entry.path();
        if entry.file_type().is_ok_and(|ty| ty.is_dir()) {
            collect_dep_info_matches(&path, needle, threshold, listed);
            continue;
        }
        if path.extension().and_then(|e| e.to_str()) != Some("d") {
            continue;
        }
        if !was_written_since(&path, threshold) {
            continue;
        }
        if std::fs::read_to_string(&path)
            .is_ok_and(|content| normalize_dep_info_content(&content).contains(needle))
        {
            *listed += 1;
        }
    }
}

/// Whether `path`'s modification time is at or after `threshold`.
///
/// A file whose timestamp cannot be read is treated as fresh. The alternative
/// would be to fail the assertion for a filesystem that does not report
/// modification times, turning a diagnostic gap into a false failure; the
/// content check still has to match, so the risk of admitting a stale artefact
/// is confined to that unusual platform.
fn was_written_since(path: &StdPath, threshold: SystemTime) -> bool {
    let Ok(modified) = std::fs::metadata(path).and_then(|metadata| metadata.modified()) else {
        return true;
    };
    modified >= threshold
}

#[cfg(test)]
mod tests {
    //! Unit tests for dep-info path normalization and freshness.
    //!
    //! Exercises [`normalize_dep_info_content`],
    //! [`normalize_staged_feature_path`], [`freshness_threshold`] and
    //! [`collect_dep_info_matches`] from the parent [`tracking`](super) module,
    //! which proves that a trybuild run reached the macro-emitted tracking
    //! binding by finding it in a dep-info this run wrote. The parent module's
    //! one public entry point
    //! (`assert_trybuild_tracking_registered_in_dep_info`) walks the real
    //! target tree and asserts; these cases take the same pieces apart so a
    //! rule about separators, escaping or timestamps can fail on its own
    //! instead of as an assertion about a trybuild run.

    use std::{
        fs,
        io,
        time::{Duration, SystemTime},
    };

    use rstest::rstest;

    use super::{
        DEP_INFO_CLOCK_SLACK,
        collect_dep_info_matches,
        freshness_threshold,
        normalize_dep_info_content,
        normalize_staged_feature_path,
    };

    /// A temporary directory and the instant a simulated run started.
    ///
    /// Returned together because every freshness case needs both and neither
    /// may be created before the other's meaning is fixed: a directory made
    /// later than `run_started` would still work, but one made earlier would
    /// let a case confuse "when the fixture was built" with "when the run
    /// began".
    ///
    /// It is fallible rather than panicking: creating the directory is setup,
    /// and a failure there is a fact about the host worth reporting as itself.
    /// The `?` belongs in the test body, where the error can name which setup
    /// failed — an `expect` here would turn an unwritable temporary directory
    /// into a panic attributed to the assertion under test.
    #[rstest::fixture]
    fn prepared_directory() -> io::Result<(tempfile::TempDir, SystemTime)> {
        let directory = tempfile::tempdir()?;
        let run_started = SystemTime::now();
        Ok((directory, run_started))
    }

    #[test]
    fn unescapes_spaces_before_normalizing_separators() {
        assert_eq!(
            normalize_dep_info_content(r"C:\workspace\feature\ file\:name.feature"),
            "c:/workspace/feature file:name.feature"
        );
    }

    #[test]
    fn normalized_dep_info_contains_normalized_unc_staged_feature() {
        let staged_feature = r"\\server\share\\tests\trybuild\rstest-bdd\tracking.feature";
        let dep_info = r"fixture: \\server\share\tests\\trybuild\rstest-bdd\tracking.feature";

        assert!(
            normalize_dep_info_content(dep_info)
                .contains(&normalize_staged_feature_path(staged_feature))
        );
    }

    /// The needle the matching cases below look for.
    const NEEDLE: &str = "staged/tracking.feature";

    /// Write a dep-info listing `NEEDLE` under `dir`, at `modified`.
    ///
    /// The timestamp is set explicitly rather than by writing and waiting, so a
    /// case can place a file confidently outside the slack in both directions.
    /// `filetime` is not a dependency of this crate; `std` has no setter, so a
    /// file whose mtime cannot be set is reported to the caller instead of
    /// silently treated as fresh by the matcher.
    fn write_dep_info(
        dir: &std::path::Path,
        name: &str,
        modified: SystemTime,
    ) -> std::path::PathBuf {
        let path = dir.join(name);
        fs::write(&path, format!("fixture: {NEEDLE}\n")).expect("dep-info");
        let file = fs::File::options()
            .write(true)
            .open(&path)
            .expect("open for timestamp");
        file.set_modified(modified).expect("set modification time");
        path
    }

    /// How far before the run a dep-info's timestamp is placed.
    ///
    /// Named so the case table can state the classification each placement
    /// expects; a bare `Duration` in the table would not say why the row
    /// belongs. `stale` is the pre-run case in every case below: no row needs
    /// an hour, so this is the single placement the three timestamps share.
    #[derive(Clone, Copy)]
    enum Written {
        /// At the instant the run started.
        During,
        /// Inside the clock slack before the run.
        InsideSlack,
        /// Well before the run: an earlier run's artefact.
        Before,
    }

    impl Written {
        /// The modification time this placement names, given the run start.
        fn modified(self, run_started: SystemTime) -> SystemTime {
            match self {
                Self::During => run_started,
                Self::InsideSlack => run_started
                    .checked_sub(DEP_INFO_CLOCK_SLACK / 2)
                    .expect("half the slack before the run"),
                Self::Before => run_started
                    .checked_sub(Duration::from_secs(60 * 60))
                    .expect("an hour before the run"),
            }
        }

        /// The file name this placement writes.
        fn file_name(self) -> &'static str {
            match self {
                Self::During => "current.d",
                Self::InsideSlack => "slightly-early.d",
                Self::Before => "previous.d",
            }
        }
    }

    /// The freshness rule in one table: which timestamps count as this run's.
    ///
    /// Three rows, and each is a distinct part of the rule rather than a
    /// repetition of one. A file written during the run is the ordinary
    /// positive. A file written an hour earlier is the stale-artefact case the
    /// freshness rule exists for: its content matches, so only the timestamp
    /// tells the two apart. A file inside the slack is the case the slack
    /// exists for in the other direction — a coarse-timestamp host stamps a
    /// file it wrote promptly with a time slightly before the run, and
    /// rejecting that would fail a correct run.
    ///
    /// Kept as one table because the three differ only in the placement and
    /// the expectation; the setup, scan and count are identical, and separate
    /// bodies let them diverge. The exact-threshold case below stays separate:
    /// it asserts the rule's *definition* rather than classifying a file.
    #[rstest]
    #[case::written_during_the_run(
        Written::During,
        1,
        "a dep-info written during this run must satisfy the assertion"
    )]
    #[case::written_before_the_run(
        Written::Before,
        0,
        "an earlier run's dep-info must not satisfy the assertion merely because its content matches: that is the stale-artefact case the freshness rule exists for"
    )]
    #[case::inside_the_clock_slack(
        Written::InsideSlack,
        1,
        "a coarse-timestamp host must not fail the assertion for a file it wrote promptly"
    )]
    fn the_freshness_rule_classifies_a_dep_info_by_when_it_was_written(
        prepared_directory: io::Result<(tempfile::TempDir, SystemTime)>,
        #[case] written: Written,
        #[case] expected: usize,
        #[case] reason: &str,
    ) {
        let (directory, run_started) =
            prepared_directory.expect("a temporary directory for the dep-info cases");
        write_dep_info(
            directory.path(),
            written.file_name(),
            written.modified(run_started),
        );
        let mut listed = 0;

        collect_dep_info_matches(
            directory.path(),
            NEEDLE,
            freshness_threshold(run_started),
            &mut listed,
        );

        assert_eq!(listed, expected, "{reason}");
    }

    /// The threshold is the run start minus the slack, not the run start.
    ///
    /// The three cases above classify files correctly under either rule, so
    /// without this one the slack could be dropped and the suite would still
    /// pass — on a host with fine timestamps, which is every host the suite
    /// normally runs on.
    #[test]
    fn the_freshness_threshold_is_the_run_start_minus_the_slack() {
        let now = SystemTime::now();
        assert_eq!(
            freshness_threshold(now),
            now.checked_sub(DEP_INFO_CLOCK_SLACK).expect("within range"),
            "the comparison the matcher uses must be the documented one, not merely any value that happens to classify the files above correctly"
        );
    }
}
