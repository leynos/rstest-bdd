//! A stamp speaks only for the family that wrote it.
//!
//! One Cargo profile can be written by several families at once. The
//! rebuild-invalidation experiment runs `cargo clean -p` on its own fixture
//! before every baseline *by design*, so a family whose stamp recorded the
//! whole shared profile vouched for a sibling's units and was refused for ever
//! after that clean: the replacement generations were themselves recorded, so
//! no name-level rule could tell the designed clean from an eviction.
//!
//! These cases drive the boundary the fix draws, from both sides. A sibling's
//! units are not this family's to claim, and the family's own units still are.

use std::{fs, path::Path};

use super::{profile_of, write_store, write_units};
use crate::nested_cargo::{
    Preparation,
    ProfileDestination,
    Sources,
    env_from_vars,
    preparation::{
        artefacts::{all_present, record},
        stamp::Stamp,
    },
};

/// The package the family under test builds.
const OWN: &str = "rstest-bdd-feature-addition-fixture";

/// The package a sibling family builds into the same profile.
const SIBLING: &str = "rstest-bdd-rebuild-invalidation-fixture";

/// The metadata hash both fixture units carry.
const HASH: &str = "0123456789abcdef";

/// The stem Cargo derives for a package's unit, underscored as a crate name is.
fn unit_stem(package: &str) -> String { format!("{}-{HASH}", package.replace('-', "_")) }

/// Describe a family rooted at `root` that builds into `target` and claims
/// `packages`.
fn owning(root: &Path, target: &Path, packages: &[&str]) -> std::io::Result<Preparation> {
    let environment = env_from_vars(Vec::new(), Some(target), ProfileDestination::Caller)?;
    let sources = Sources::new("unit", "v", "hash");
    let mut preparation =
        Preparation::new(sources, root.to_path_buf(), environment, root.to_path_buf())?;
    preparation.owned_packages = packages
        .iter()
        .map(|package| (*package).to_owned())
        .collect();
    Ok(preparation)
}

/// A profile both packages built into, with one unit and one fingerprint
/// record each.
///
/// `packages` is what the family under test declares it built; the two fixture
/// units are written either way, exactly as a shared profile would hold them.
fn shared_profile(packages: &[&str]) -> std::io::Result<(tempfile::TempDir, Preparation)> {
    let directory = tempfile::tempdir()?;
    let target = directory.path().join("target");
    let profile = target.join("debug");
    write_units(
        &profile.join("deps"),
        &[(&unit_stem(OWN), "rlib"), (&unit_stem(SIBLING), "rlib")],
    )?;
    write_store(&profile.join(".fingerprint"), OWN)?;
    write_store(&profile.join(".fingerprint"), SIBLING)?;
    let preparation = owning(directory.path(), &target, packages)?;
    Ok((directory, preparation))
}

/// Remove what `cargo clean -p SIBLING` removes: its unit and its records.
fn clean_sibling(profile: &Path) -> std::io::Result<()> {
    let deps = profile.join("deps");
    let stem = unit_stem(SIBLING);
    fs::remove_file(deps.join(format!("{stem}.d")))?;
    fs::remove_file(deps.join(format!("lib{stem}.rlib")))?;
    let store = profile.join(".fingerprint");
    fs::remove_dir_all(store.join(format!("{SIBLING}-{HASH}")))
}

/// The sibling's units are not recorded, and the family's own are.
#[test]
fn a_sibling_packages_units_are_not_recorded() -> std::io::Result<()> {
    let (_directory, preparation) = shared_profile(&[OWN])?;
    let recorded = record(&preparation).expect("record");
    let names = recorded.first().expect("the fixture describes one profile");
    assert!(
        names
            .iter()
            .all(|name| !name.to_lowercase().contains("rebuild")),
        "the sibling's units are its own family's to vouch for: {names:?}"
    );
    // Both halves of the family's own claim survive the filter: its compiled
    // output, and the record Cargo reads to decide that output is current.
    assert!(
        names.iter().any(|name| {
            Path::new(name)
                .extension()
                .is_some_and(|extension| extension.eq_ignore_ascii_case("rlib"))
        }),
        "the family's own output must still be recorded: {names:?}"
    );
    assert!(
        names.iter().any(|name| name.contains("feature-addition")),
        "the family's own fingerprint record must still be recorded: {names:?}"
    );
    Ok(())
}

/// A sibling's designed clean leaves this family's stamp valid.
///
/// The regression: the rebuild experiment cleans its own fixture before every
/// baseline, which used to strand the addition family's stamp with sixteen
/// missing names the tree could never restore.
#[test]
fn a_sibling_packages_clean_does_not_refuse_the_stamp() -> std::io::Result<()> {
    let (_directory, preparation) = shared_profile(&[OWN])?;
    let recorded = record(&preparation).expect("record");
    let profile = profile_of(&preparation)?;
    assert!(
        all_present(&preparation, None).expect("structural check"),
        "the fixture must start usable"
    );

    clean_sibling(&profile)?;

    assert!(
        all_present(&preparation, Some(&Stamp { recorded })).expect("check"),
        "a clean of a package this family never built must not refuse its stamp"
    );
    Ok(())
}

/// The family's own units are still required.
///
/// The other side of the boundary: scoping the record must not let a family
/// vouch for nothing. Removing its own output is still an eviction.
#[test]
fn the_family_s_own_units_are_still_required() -> std::io::Result<()> {
    let (_directory, preparation) = shared_profile(&[OWN])?;
    let recorded = record(&preparation).expect("record");
    let profile = profile_of(&preparation)?;

    let stem = unit_stem(OWN);
    fs::remove_file(profile.join("deps").join(format!("lib{stem}.rlib")))?;
    assert!(
        profile.join("deps").join(format!("{stem}.d")).is_file(),
        "the unit's record must survive, so the structural layer cannot refuse instead"
    );

    assert!(
        !all_present(&preparation, Some(&Stamp { recorded })).expect("check"),
        "an eviction of the family's own work must still refuse the stamp"
    );
    Ok(())
}

/// A family that names no package still vouches for the whole profile.
///
/// That is what [`Preparation::for_fixture`] relies on, since it sets the
/// child's `CARGO_TARGET_DIR` to the directory it stamps. It is also the state
/// the regression above was measured in, so the case is kept as the before
/// picture: the sibling's clean refuses this stamp.
#[test]
fn a_family_that_names_no_package_claims_the_whole_profile() -> std::io::Result<()> {
    let (_directory, preparation) = shared_profile(&[])?;
    let recorded = record(&preparation).expect("record");
    let names = recorded.first().expect("the fixture describes one profile");
    assert!(
        names
            .iter()
            .any(|name| name.to_lowercase().contains("rebuild")),
        "with no declared ownership every name in the profile is recorded: {names:?}"
    );

    let profile = profile_of(&preparation)?;
    clean_sibling(&profile)?;
    assert!(
        !all_present(&preparation, Some(&Stamp { recorded })).expect("check"),
        "this is the failure the declared ownership removes: the sibling's designed clean refuses \
         a stamp that claimed its units"
    );
    Ok(())
}
