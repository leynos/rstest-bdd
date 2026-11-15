//! Regression coverage for how the trybuild target root is discovered.
//!
//! The root is what both staged inputs and inspected artefacts hang off, so
//! resolving it wrongly points trybuild at a tree that holds neither. It used
//! to be read as "two levels above the executable, which must sit in `deps`",
//! and Cargo 1.100.0 retired that premise by moving every compiled unit to
//! `<profile>/build/<pkg>/<hash>/out`.

use rstest::rstest;

use super::*;

/// A target root that Cargo would have tagged, holding `executable` beneath it.
///
/// The marker is what makes the root discoverable, so a fixture without one
/// would test the absence path rather than the layout under test.
fn tagged_root(structure: &[&str]) -> (tempfile::TempDir, Utf8PathBuf) {
    let directory = tempfile::tempdir().expect("temp dir");
    let root = Utf8PathBuf::from_path_buf(directory.path().to_path_buf()).expect("utf-8 path");
    std::fs::write(
        root.join("CACHEDIR.TAG"),
        b"Signature: 8a477f597d28d172789f06886806bc55",
    )
    .expect("cache tag");
    let mut executable = root.clone();
    for part in structure {
        executable.push(part);
    }
    std::fs::create_dir_all(executable.parent().expect("executable parent"))
        .expect("executable dir");
    std::fs::write(executable.as_std_path(), b"binary").expect("executable");
    (directory, executable)
}

/// Both layouts resolve to the tagged root, whatever directory holds the
/// executable.
///
/// The classic shape put integration tests in `deps`; Cargo 1.100.0 moved them
/// to `build/<pkg>/<hash>/out`. Naming either one would break the other, which
/// is exactly the regression this covers.
#[rstest]
#[case::classic_deps(&["debug", "deps", "trybuild_macros-a1b2c3"])]
#[case::build_dir_out(&[
    "debug",
    "build",
    "rstest-bdd",
    "ce1892561403e9e7",
    "out",
    "trybuild_macros-ce1892561403e9e7",
])]
#[case::coverage_deps(&["llvm-cov-target", "debug", "deps", "trybuild_macros-a1b2c3"])]
#[case::coverage_build_dir_out(&[
    "llvm-cov-target",
    "debug",
    "build",
    "rstest-bdd",
    "ce1892561403e9e7",
    "out",
    "trybuild_macros-ce1892561403e9e7",
])]
fn derives_target_root_from_test_executable(#[case] structure: &[&str]) {
    let (directory, executable) = tagged_root(structure);
    let expected = Utf8PathBuf::from_path_buf(directory.path().to_path_buf()).expect("utf-8 path");

    assert_eq!(
        target_directory_from_test_executable(executable.as_path()),
        Some(expected),
        "the tagged root must be found from {executable}"
    );
}

/// An executable outside Cargo's layouts resolves only through a tag above it.
///
/// The lookup does not parse the path, so these shapes are not rejected for
/// their form: each is refused because no ancestor carries `CACHEDIR.TAG`.
/// That distinction is what lets `build/<pkg>/<hash>/out` work without this
/// module having to name it, so the case pins the mechanism rather than a list
/// of accepted directories.
#[rstest]
#[case::unrecognized_directory(&["debug", "examples", "trybuild_macros-a1b2c3"])]
#[case::out_without_build_directory(&[
    "debug",
    "other",
    "rstest-bdd",
    "d195883d3c395ab2",
    "out",
    "trybuild_macros-d195883d3c395ab2",
])]
fn an_untagged_layout_yields_no_root(#[case] structure: &[&str]) {
    let directory = tempfile::tempdir().expect("temp dir");
    let tagged_ancestor = directory
        .path()
        .ancestors()
        .skip(1)
        .find(|ancestor| ancestor.join("CACHEDIR.TAG").is_file());
    if tagged_ancestor.is_some() {
        return;
    }
    let mut executable = directory.path().to_path_buf();
    for part in structure {
        executable.push(part);
    }
    std::fs::create_dir_all(executable.parent().expect("executable parent"))
        .expect("executable dir");
    std::fs::write(&executable, b"binary").expect("executable");

    assert_eq!(
        target_directory_from_test_executable(Utf8Path::new(
            executable.to_str().expect("utf-8 path")
        )),
        None,
        "without a tag above it, {executable:?} must yield no root"
    );
}

/// A tree with no cache tag yields no root, so the caller falls back rather
/// than guessing a directory.
///
/// The lookup walks every ancestor, not just the temporary directory, so this
/// is only a "no tag anywhere above it" case if the temporary directory has no
/// tagged ancestor. It does when `TMPDIR` points beneath a Cargo target
/// directory, which is common on CI: the assertion then fails for the
/// environment rather than for the code. The guard states that precondition
/// instead of letting the case report a false defect. The tagged case above
/// covers the positive direction, so nothing is lost by skipping here.
///
/// The guard returns early, matching the suite's other environment-gated cases
/// (`gpui_macro_expansions_match_snapshots`, `should_seed_dump_steps`): the
/// reason lives in this comment rather than on a stream, because the workspace
/// denies `print_stderr`. Set `TMPDIR` outside a Cargo target directory to
/// exercise the case; the tagged case above still covers the positive
/// direction on any host, so a skipped run loses no coverage of the lookup
/// itself.
#[test]
fn an_untagged_tree_yields_no_target_root() {
    let directory = tempfile::tempdir().expect("temp dir");
    let tagged_ancestor = directory
        .path()
        .ancestors()
        .skip(1)
        .find(|ancestor| ancestor.join("CACHEDIR.TAG").is_file());
    if tagged_ancestor.is_some() {
        return;
    }
    let executable = directory.path().join("debug/deps/trybuild_macros-a1b2c3");
    std::fs::create_dir_all(executable.parent().expect("parent")).expect("executable dir");

    assert_eq!(
        target_directory_from_test_executable(Utf8Path::new(
            executable.to_str().expect("utf-8 path")
        )),
        None
    );
}

/// The live run resolves its own target root, so the discovery works against
/// the layout this toolchain actually produces rather than only the fixtures.
#[test]
#[serial_test::serial(trybuild_target_directory)]
fn target_directory_uses_running_test_executable() {
    let workspace_root = Utf8Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Utf8Path::parent)
        .expect("workspace root must be two levels above the manifest directory");
    let expected = target_directory_from_running_test_executable().expect(
        "integration tests should run beneath a target directory carrying Cargo's `CACHEDIR.TAG`",
    );

    assert_eq!(trybuild_target_directory(workspace_root), expected);
}
