//! Fixed cases that belong beside the properties.
//!
//! Two rules the generated cases depend on but cannot themselves establish:
//! which names the generator must refuse to spell, and that the stamp a family
//! writes beside its inputs is not one of them. Kept in a file of their own so
//! the property module stays within the repository's line budget.

use std::fs;

use super::{
    super::{Directories, Preparation, Sources, input_hash, stamp::stamp_path},
    is_windows_reserved,
};

/// The generator refuses exactly the Windows device names, and no others.
///
/// Run 43617094223 failed on `tree = { "nul/a": [] }`: `write_tree` reached
/// `fs::write` and Windows resolved `nul` as a device, so no directory could be
/// created for it. A property generator that can spell such a name will
/// eventually spell it, so the exclusion has to be complete rather than lucky.
/// The positive half is what keeps the filter from over-reaching: `nul_`,
/// `null`, `com0` and `lpt` are ordinary names, and rejecting them would lose
/// real coverage.
#[test]
fn windows_device_names_are_the_only_segments_excluded() {
    for reserved in [
        "con", "prn", "aux", "nul", "com1", "com9", "lpt1", "lpt9", "CON", "Nul", "COM5",
    ] {
        assert!(
            is_windows_reserved(reserved),
            "{reserved} is a Windows device name and cannot be created as a file"
        );
    }
    for ordinary in [
        "nul_", "null", "con_", "console", "com0", "com10", "lpt", "lpt0", "lpt10", "a", "target",
    ] {
        assert!(
            !is_windows_reserved(ordinary),
            "{ordinary} is an ordinary name and excluding it would lose coverage"
        );
    }
}

/// The stamp's own file is not an input, so a family that stamps a root does
/// not thereby invalidate its own fingerprint.
#[test]
fn a_stamp_beside_the_inputs_does_not_change_the_hash() {
    let directory = tempfile::tempdir().expect("temp dir");
    let root = directory.path().to_path_buf();
    fs::write(root.join("feature.feature"), b"Feature: one").expect("input");

    let before = input_hash(std::slice::from_ref(&root)).expect("hash before");
    // The name is the one `write_stamp` derives for a family rooted here.
    let environment = crate::nested_cargo::env_from_vars(
        Vec::new(),
        Some(&std::env::temp_dir().join("target")),
        crate::nested_cargo::ProfileDestination::Caller,
    )
    .expect("the fixture environment should resolve");
    let preparation = Preparation::new(
        Sources::new("unit", "v", "hash"),
        Directories::new(root.clone(), root.clone()),
        environment,
    )
    .expect("the fixture preparation should describe its artefacts");
    let stamp = stamp_path(&preparation);
    assert_eq!(stamp.parent(), Some(root.as_path()), "stamp beside inputs");
    fs::write(&stamp, b"fingerprint").expect("stamp");
    let after = input_hash(std::slice::from_ref(&root)).expect("hash after");

    assert_eq!(
        before, after,
        "the stamp records the fingerprint of these inputs, so it cannot be one of them"
    );
}
