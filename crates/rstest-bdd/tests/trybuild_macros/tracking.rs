//! Tracking-binding dep-info assertion for the staged trybuild fixtures.
//!
//! After the trybuild run has compiled the compile-pass fixtures, this module
//! proves at least one dep-info under the trybuild build tree lists the staged
//! `tracking.feature`. It is the cheap mid-tier signal that catches a future
//! codegen refactor silently dropping the binding, without depending on the
//! expensive nested-cargo regression test (`ExecPlan` Milestone 4).

use std::path::Path as StdPath;

use camino::{Utf8Path, Utf8PathBuf};

use crate::staging::trybuild_target_directory;

/// Assert that the staged compile-pass fixtures registered the tracking
/// binding: at least one dep-info under the trybuild build tree lists the
/// staged `tracking.feature`.
///
/// This is the cheap mid-tier signal that catches a future codegen refactor
/// silently dropping the binding, without depending on the expensive
/// nested-cargo regression test. The check scans the trybuild artefacts from
/// the run we have already paid for (see the `ExecPlan` Milestone 4 section).
pub(crate) fn assert_trybuild_tracking_registered_in_dep_info() {
    let target_directory = workspace_target_directory();
    let staged_feature = target_directory.join("tests/trybuild/rstest-bdd/tracking.feature");
    let needle = normalize_staged_feature_path(staged_feature.as_str());
    let mut listed = 0;
    collect_dep_info_matches(target_directory.as_std_path(), &needle, &mut listed);
    assert!(
        listed > 0,
        "no trybuild dep-info below {target_directory} lists the staged `{needle}`; the \
         macro-emitted tracking binding is not reaching dep-info"
    );
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

/// Recursively count `*.d` files under `dir` whose content contains `needle`.
///
/// Walking the whole trybuild tree keeps the assertion independent of
/// trybuild's per-target directory naming (the project dir hosts a
/// host-triplet subdirectory).
fn collect_dep_info_matches(dir: &StdPath, needle: &str, listed: &mut usize) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries {
        let Ok(entry) = entry else {
            continue;
        };
        let path = entry.path();
        if entry.file_type().is_ok_and(|ty| ty.is_dir()) {
            collect_dep_info_matches(&path, needle, listed);
            continue;
        }
        if path.extension().and_then(|e| e.to_str()) != Some("d") {
            continue;
        }
        if std::fs::read_to_string(&path)
            .is_ok_and(|content| normalize_dep_info_content(&content).contains(needle))
        {
            *listed += 1;
        }
    }
}

#[cfg(test)]
mod tests {
    //! Unit tests for dep-info path normalization.

    use super::{normalize_dep_info_content, normalize_staged_feature_path};

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
}
