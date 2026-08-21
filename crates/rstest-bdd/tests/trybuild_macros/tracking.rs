//! Tracking-binding dep-info assertion for the staged trybuild fixtures.
//!
//! After the trybuild run has compiled the compile-pass fixtures, this module
//! proves at least one dep-info under the trybuild build tree lists the staged
//! `basic.feature`. It is the cheap mid-tier signal that catches a future
//! codegen refactor silently dropping the binding, without depending on the
//! expensive nested-cargo regression test (`ExecPlan` Milestone 4).

use std::path::{Path as StdPath, PathBuf};

/// Assert that the staged compile-pass fixtures registered the tracking
/// binding: at least one dep-info under the trybuild build tree lists the
/// staged `basic.feature`.
///
/// This is the cheap mid-tier signal that catches a future codegen refactor
/// silently dropping the binding, without depending on the expensive
/// nested-cargo regression test. The check scans the trybuild artefacts from
/// the run we have already paid for (see the `ExecPlan` Milestone 4 section).
pub(crate) fn assert_trybuild_tracking_registered_in_dep_info() {
    let workspace_target = workspace_target_directory();
    let staged_feature = workspace_target.join("tests/trybuild/rstest-bdd/basic.feature");
    let needle = staged_feature
        .to_string_lossy()
        .replace('\\', "/")
        .to_lowercase();
    let mut listed = 0;
    collect_dep_info_matches(&workspace_target, &needle, &mut listed);
    assert!(
        listed > 0,
        "no trybuild dep-info below {} lists the staged `{needle}`; \
         the macro-emitted tracking binding is not reaching dep-info",
        workspace_target.display()
    );
}

/// Return the workspace target directory containing trybuild's copied source.
///
/// Trybuild keeps its generated fixture crate here. Coverage may direct the
/// corresponding compiler artefacts to a subdirectory, so the assertion scans
/// this entire root for the staged path's dep-info entry.
fn workspace_target_directory() -> PathBuf {
    let crate_dir = StdPath::new(env!("CARGO_MANIFEST_DIR"));
    crate_dir.parent().and_then(StdPath::parent).map_or_else(
        || panic!("workspace root must be two levels above the manifest dir"),
        |workspace_root| workspace_root.join("target"),
    )
}

/// Normalize a dep-info's raw text the same way as the needle — `/`
/// separators, folded case — so the Windows CI leg (backslash-separated,
/// drive-letter-cased paths) matches the staged feature path deterministically.
fn normalize_dep_info_content(content: &str) -> String {
    content
        .replace("\\:", ":")
        .replace('\\', "/")
        .replace("//", "/")
        .to_lowercase()
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
