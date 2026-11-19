//! What the real gate does with the budget it is handed.
//!
//! The cases in `deadline/tests.rs` state the arithmetic on the budget type.
//! These state the property on the *production* path: `instruments_within`
//! binds the real toolchain probe, and the gate consults it once, so a budget
//! that is already spent must stop the gate before any probe is spawned — and
//! it must report the deadline rather than a failed `cargo -vV`.
//!
//! This matters because the arithmetic test keeps passing if the gate stops
//! consulting the budget: the type would still be right, and nothing would say
//! the gate had stopped asking. The probe here is the production binding, not
//! a substitute; a fabricated probe would test the substitution rather than
//! the path the review finding is about.

use std::{io, time::Duration};

use super::{environment_in, populate_profile, preparation};
use crate::nested_cargo::{
    Deadline,
    PreparationSummary,
    ensure_prepared_reported,
    instruments_within,
};

/// A spent budget refuses the gate, and names the probe it never started.
#[test]
fn a_spent_gate_budget_refuses_before_the_toolchain_probe_runs() {
    let directory = tempfile::tempdir().expect("a temporary directory");
    let target = directory.path().join("target");
    populate_profile(&target).expect("a populated profile");
    let environment = environment_in(&target).expect("the fixture environment should resolve");
    let family = preparation(directory.path(), &environment, "v", "hash")
        .expect("the fixture preparation should describe its artefacts");

    let error = ensure_prepared_reported(
        &family,
        // The lazy runner, so a gate that got as far as the stamp would run
        // the closure rather than fail fast with the nextest message. The
        // refusal must come from the budget instead of from either.
        false,
        instruments_within(&family, Deadline::starting_now(Duration::ZERO)),
        || panic!("a spent budget must refuse before any preparation is started"),
    )
    .expect_err("a spent budget must refuse the gate");

    assert_eq!(
        error.kind(),
        io::ErrorKind::TimedOut,
        "the gate must report the deadline rather than a failed probe: {error}"
    );
    let text = error.to_string();
    assert!(
        text.contains("cargo -vV"),
        "the refusal must name the probe the gate declined to start: {text}"
    );
}

/// A live budget lets the gate run the probe and then judge the tree.
#[test]
fn a_live_gate_budget_reaches_the_preparation_rather_than_the_deadline() {
    // The successful path. An unpopulated tree has no stamp, so the gate must
    // run the closure — which is where a binding that refused whenever it was
    // handed a deadline would fail while the case above still passed.
    let directory = tempfile::tempdir().expect("a temporary directory");
    let target = directory.path().join("target");
    populate_profile(&target).expect("a populated profile");
    let environment = environment_in(&target).expect("the fixture environment should resolve");
    let family = preparation(directory.path(), &environment, "v", "hash")
        .expect("the fixture preparation should describe its artefacts");

    let mut prepared = false;
    ensure_prepared_reported(
        &family,
        false,
        instruments_within(&family, Deadline::starting_now(Duration::from_secs(600))),
        || {
            prepared = true;
            Ok(PreparationSummary::default())
        },
    )
    .expect("a live budget over a cold family must prepare it");

    assert!(
        prepared,
        "the gate must reach the preparation once the probe has run within budget"
    );
}
