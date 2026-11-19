//! The withdrawal the gate performs when its witness refutes a stamp.
//!
//! Kept out of the parent module so the family's description, its gate and
//! this regression each stay readable. The helpers here build the shapes the
//! real family uses, so the case below reads the profile and the stamp
//! exactly where the gate does.

use super::{ensure_gated_with, nested_cargo};

/// The directory holding the fixture's compiled units, and the stamp file.
///
/// Both are the shapes the real family uses: the profile is
/// [`nested_cargo::DEFAULT_PROFILE`] below the environment's target root,
/// and the stamp is named after the family, so the case under test reads
/// them exactly where the gate does.
fn scratch_family(
    scratch: &std::path::Path,
) -> std::io::Result<(
    nested_cargo::Preparation,
    std::path::PathBuf,
    std::path::PathBuf,
)> {
    let target = scratch.join("target");
    let profile = target.join(nested_cargo::DEFAULT_PROFILE);
    std::fs::create_dir_all(profile.join(".fingerprint"))?;
    std::fs::write(profile.join(".fingerprint/unit"), b"record")?;
    // A full, plausible unit directory — just not this fixture's. This is
    // the state a preparation that compiled the dependency graph but never
    // reached the fixture's own test targets leaves behind.
    //
    // Each unit is written the way rustc names it: the `.d` record and the
    // `.rlib` share one hashed stem. A fixture that wrote an unsuffixed
    // record beside a hashed output would be refused by the pairing rule
    // before the witness ran, and the case would pass for the wrong reason.
    let deps = profile.join("deps");
    std::fs::create_dir_all(&deps)?;
    for stem in ["libdep_a", "libdep_b"] {
        let unit = format!("{stem}-0123456789abcdef");
        std::fs::write(deps.join(format!("{unit}.d")), b"dep-info")?;
        std::fs::write(deps.join(format!("{unit}.rlib")), b"unit")?;
    }

    let environment = nested_cargo::env_from_vars(
        Vec::new(),
        Some(&target),
        nested_cargo::ProfileDestination::Caller,
    )?;
    let preparation = nested_cargo::Preparation::new(
        nested_cargo::Sources::new("cargo-bdd-cli", "test", "hash"),
        scratch.join("stamp-root"),
        environment,
        scratch.join("stamp-root"),
    )?;
    let stamp = preparation
        .directory
        .join(".cargo-bdd-cli.rstest-bdd-prepared");
    Ok((preparation, profile, stamp))
}

/// Write the fixture's own test binaries where the witness looks for them.
fn write_fixture_binaries(profile: &std::path::Path) -> std::io::Result<()> {
    let deps = profile.join("deps");
    std::fs::create_dir_all(&deps)?;
    for stem in super::witness::FIXTURE_TEST_BINARIES {
        std::fs::write(deps.join(format!("{stem}-0123456789abcdef")), b"binary")?;
    }
    Ok(())
}

#[test]
fn a_refuted_stamp_is_withdrawn_so_the_next_run_can_prepare() {
    // The hole a stamp-only gate leaves. `prepare_now` writes the stamp
    // inside the preparation, before the witness can be read, and it
    // returns early from a matching stamp on the next run without preparing
    // at all. So a tree whose binaries were evicted after being stamped
    // still carries a valid stamp, and every timed assertion behind it would
    // run against nothing to inspect.
    //
    // This drives `ensure_gated_with`, which every timed test reaches
    // through `ensure_cli_prepared` and which `nested_fixture_preparation`
    // reaches through the same `witness_or_withdraw`, rather than the
    // predicate it calls: asserting on the predicate alone would still pass
    // for a gate that had stopped consulting it, and the gate is what
    // admits the timed assertions. The preparation closure runs (under
    // `cargo test`), finds the profile already structurally usable and
    // stamps it — so the tree it leaves behind is exactly the state under
    // test: a *valid* stamp over units that are not the fixture's.
    let scratch = tempfile::tempdir().expect("temp dir");
    let (preparation, profile, stamp) =
        scratch_family(scratch.path()).expect("the scratch family's tree should be written");

    // A preparation that succeeds while doing no work at all: the point is
    // that the stamp it then writes vouches for a tree holding none of the
    // fixture's own units.
    let error = ensure_gated_with(&preparation, false, || {
        Ok(nested_cargo::PreparationSummary::default())
    })
    .expect_err("a preparation that leaves no fixture binaries must not satisfy the gate");
    assert!(
        error.to_string().contains("test binaries"),
        "the failure must name what is missing, got: {error}"
    );

    // The refusal alone leaves the stamp in place, and both entry points
    // return early from a matching stamp — so without this the failure is
    // permanent: every later attempt reads the same stamp and reports the
    // same missing binaries, and the preparation never runs again.
    assert!(
        !stamp.exists(),
        "the refuted stamp at {} must be withdrawn, or every later run repeats this failure",
        stamp.display()
    );

    // Recovery, with a callback that does the work the failed one skipped.
    // Driving the gate again is what proves the withdrawal made a
    // difference rather than only tidying the tree: the stamp is gone, so
    // the preparation runs and the witness accepts what it left.
    ensure_gated_with(&preparation, false, || {
        write_fixture_binaries(&profile)
            .expect("the recovery preparation should leave the fixture binaries");
        Ok(nested_cargo::PreparationSummary::default())
    })
    .expect("a preparation that leaves the fixture binaries must recover the family");
    assert!(
        stamp.is_file(),
        "the recovered preparation must stamp the profile it vouched for"
    );

    // And the recovered stamp is the one a later run reuses. The closure
    // must not run here: if it did, the timing would hide a gate that had
    // simply stopped finding the stamp it wrote.
    let mut prepared_again = false;
    ensure_gated_with(&preparation, true, || {
        prepared_again = true;
        Ok(nested_cargo::PreparationSummary::default())
    })
    .expect("the recovered family must satisfy the gate without preparing again");
    assert!(
        !prepared_again,
        "a later run must reuse the recovered stamp instead of preparing again"
    );
}
