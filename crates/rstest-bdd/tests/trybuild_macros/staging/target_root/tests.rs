//! Regression coverage for how the trybuild target root is discovered.
//!
//! Exercises [`target_directory_from_test_executable`] and
//! [`trybuild_target_directory`] from the parent [`target_root`](super)
//! module, which resolve the Cargo target directory the staging code both
//! stages into and inspects. The root is what both staged inputs and inspected
//! artefacts hang off, so resolving it wrongly points trybuild at a tree that
//! holds neither. It used to be read as "two levels above the executable, which
//! must sit in `deps`", and Cargo 1.100.0 retired that premise by moving every
//! compiled unit to `<profile>/build/<pkg>/<hash>/out`.

use rstest::rstest;

use super::*;

/// A temporary directory's path as a UTF-8 path.
///
/// A temporary directory is not guaranteed to have a UTF-8 path, and the
/// fixtures below need one to push Cargo's layout onto. Reporting the
/// conversion failure here keeps it out of the fixtures, where it would be one
/// more closure for the complexity lint to count.
fn temp_root(directory: &tempfile::TempDir) -> std::io::Result<Utf8PathBuf> {
    Utf8PathBuf::from_path_buf(directory.path().to_path_buf())
        .map_err(|path| std::io::Error::other(format!("not utf-8: {}", path.display())))
}

/// The directory that must exist before `executable` can be written.
///
/// A path built from a non-empty structure always has a parent, so this only
/// reports for an empty one. Naming it keeps the caller free of the closure
/// that would otherwise express the same thing.
fn parent_of(executable: &Utf8Path) -> std::io::Result<&Utf8Path> {
    executable
        .parent()
        .ok_or_else(|| std::io::Error::other(format!("{executable} has no parent")))
}

/// A temporary directory opened as a capability.
///
/// The negative fixtures below create their trees through a handle on the
/// directory they own rather than through ambient `std::fs`, with every path
/// relative to it. A `Dir` refuses an absolute path or one that climbs out with
/// `..`, so a mistake in a fixture path cannot create anything outside the
/// temporary directory — which matters here because the fixtures' whole purpose
/// is to build shapes the lookup must refuse.
///
/// The `TempDir` is returned alongside the handle: `Dir` names the directory
/// but does not own it, so dropping the guard would delete the tree the test is
/// about to inspect.
fn temp_capability(
    directory: &tempfile::TempDir,
) -> std::io::Result<(cap_std::fs_utf8::Dir, Utf8PathBuf)> {
    let root = temp_root(directory)?;
    let capability = cap_std::fs_utf8::Dir::open_ambient_dir(&root, cap_std::ambient_authority())?;
    Ok((capability, root))
}

/// Write an executable at `structure` beneath `capability`, joining its parts.
///
/// `cap_std::fs_utf8::Dir` takes a relative path, so the parts are joined with
/// `/` rather than through an ambient path type.
fn write_executable(
    capability: &cap_std::fs_utf8::Dir,
    structure: &[&str],
) -> std::io::Result<Utf8PathBuf> {
    let relative = structure.join("/");
    let executable = Utf8PathBuf::from(relative.as_str());
    let parent = parent_of(&executable)?;
    capability.create_dir_all(parent)?;
    capability.write(relative.as_str(), b"binary")?;
    Ok(executable)
}

/// Whether `directory` sits beneath something Cargo tagged.
///
/// The lookup walks every ancestor rather than only the temporary directory
/// itself, so a case that expects no tag above it is only meaningful when the
/// temporary directory has no tagged ancestor. It does when `TMPDIR` points
/// beneath a Cargo target directory, which is common on CI, and the assertion
/// would then fail for the environment rather than for the code.
fn under_tagged_ancestor(directory: &tempfile::TempDir) -> bool {
    directory
        .path()
        .ancestors()
        .skip(1)
        .any(|ancestor| ancestor.join("CACHEDIR.TAG").is_file())
}

/// A target root that Cargo would have tagged, holding `executable` beneath it.
///
/// The marker is what makes the root discoverable, so a fixture without one
/// would test the absence path rather than the layout under test.
///
/// The tree is built through a handle on the temporary directory this fixture
/// owns, with every path relative to it, so a mistake in the structure cannot
/// create anything outside it — the same confinement the negative fixtures
/// below rely on.
fn tagged_root(structure: &[&str]) -> std::io::Result<(tempfile::TempDir, Utf8PathBuf)> {
    let directory = tempfile::tempdir()?;
    let (capability, root) = temp_capability(&directory)?;
    capability.write(
        "CACHEDIR.TAG",
        b"Signature: 8a477f597d28d172789f06886806bc55",
    )?;
    let relative = write_executable(&capability, structure)?;
    Ok((directory, root.join(relative)))
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
fn derives_target_root_from_test_executable(#[case] structure: &[&str]) -> std::io::Result<()> {
    let (directory, executable) = tagged_root(structure)?;
    let expected = temp_root(&directory)?;

    assert_eq!(
        target_directory_from_test_executable(executable.as_path()),
        Some(expected),
        "the tagged root must be found from {executable}"
    );
    Ok(())
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
    if under_tagged_ancestor(&directory) {
        return;
    }
    // The tree is built through a handle on this test's own temporary
    // directory, and the absolute path the lookup receives is then derived
    // from that same root, so the fixture cannot write outside it.
    let (capability, root) = temp_capability(&directory).expect("the temporary directory");
    let relative = write_executable(&capability, structure).expect("the fixture executable");
    let executable = root.join(relative);

    assert_eq!(
        target_directory_from_test_executable(executable.as_path()),
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
    if under_tagged_ancestor(&directory) {
        return;
    }
    // As above: created through a capability on this test's own directory, so
    // no path in the fixture can escape it.
    let (capability, root) = temp_capability(&directory).expect("the temporary directory");
    let relative = write_executable(&capability, &["debug", "deps", "trybuild_macros-a1b2c3"])
        .expect("the fixture executable");
    let executable = root.join(relative);

    assert_eq!(
        target_directory_from_test_executable(executable.as_path()),
        None
    );
}

/// The live run resolves its own target root, so the discovery works against
/// the layout this toolchain actually produces rather than only the fixtures.
///
/// Gated on the running executable actually having a tagged ancestor, for the
/// same reason as [`an_untagged_tree_yields_no_target_root`]: Cargo writes
/// `CACHEDIR.TAG` into the target directory it creates, but a tree assembled or
/// copied by other means need not carry one — `cargo llvm-cov --target-dir`, a
/// restored CI cache, and a hand-made `target/` all can lack it. The production
/// path then falls back to `cargo metadata` by design, which this test would
/// otherwise report as a defect in the lookup. Skipping states the precondition
/// instead; the fixture cases above already prove the lookup itself on any
/// host.
#[test]
#[serial_test::serial(trybuild_target_directory)]
fn target_directory_uses_running_test_executable() {
    let workspace_root = Utf8Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Utf8Path::parent)
        .expect("workspace root must be two levels above the manifest directory");
    let Some(expected) = target_directory_from_running_test_executable() else {
        return;
    };

    assert_eq!(trybuild_target_directory(workspace_root), expected);
}

/// The metadata fallback queries without resolving dependencies or the network.
///
/// The query only reads `target_directory`, so `--no-deps` keeps it from
/// building a resolve graph and `CARGO_NET_OFFLINE` keeps it off the network.
/// Both are asserted on the built command rather than by running Cargo.
#[test]
fn the_metadata_fallback_is_offline_and_skips_dependencies() {
    let command = metadata_command(Utf8Path::new("/workspace"));
    let cargo = command.cargo_command();
    let args = cargo
        .get_args()
        .map(|arg| arg.to_string_lossy().into_owned())
        .collect::<Vec<_>>();
    let offline = cargo
        .get_envs()
        .find(|(key, _)| *key == std::ffi::OsStr::new("CARGO_NET_OFFLINE"))
        .and_then(|(_, value)| value);

    assert!(
        args.iter().any(|arg| arg == "--no-deps"),
        "the target-root query must not build a dependency graph; got {args:?}"
    );
    assert_eq!(
        offline,
        Some(std::ffi::OsStr::new("true")),
        "the target-root query must run offline, so a cold cache cannot turn \
         this fallback into a network request"
    );
}
