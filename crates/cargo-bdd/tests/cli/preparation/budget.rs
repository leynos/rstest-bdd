//! The timed gate's one budget, from the closure query to the probes.
//!
//! The timed CLI gate runs three subprocesses before a test body starts: the
//! fixture's dependency-closure query and the two toolchain probes. They share
//! one [`nested_cargo::Deadline`], so a query that succeeds late leaves the
//! probes only what remains rather than a fresh allowance each. The functions
//! here are that hand-off. Each takes the query as a parameter, and the inner
//! two take the family's inputs as a value, so a regression can spend the
//! budget in the query without the result depending on how long the inputs
//! took to hash.

use std::{path::PathBuf, time::Duration};

use eyre::{Context, Result};

use super::{
    cli_preparation,
    ensure_gated_with,
    fixture_dir,
    nested_build,
    nested_cargo,
    owned_units,
};

/// A family description, its child environment, and the budget left for the
/// probes.
pub(super) type Described = (
    nested_cargo::Preparation,
    nested_cargo::NestedCargoEnv,
    nested_cargo::Deadline,
);

/// What [`cli_preparation`] resolves: the child environment, the target root
/// and the input hash.
pub(super) type Inputs = (nested_cargo::NestedCargoEnv, PathBuf, String);

/// The gate's deadline and the limit its closure query is held to.
///
/// The two travel together because the query is bounded by whichever is
/// sooner. For the timed gate they are the same figure; the named preparation
/// test passes its own period for both.
#[derive(Clone, Copy)]
pub(super) struct Budget {
    /// The deadline every command in the gate is charged against.
    pub(super) deadline: nested_cargo::Deadline,
    /// The most the closure query may take by itself.
    pub(super) query_limit: Duration,
}

impl Budget {
    /// A budget of `period`, starting now, whose query may use all of it.
    pub(super) fn starting_now(period: Duration) -> Self {
        Self {
            deadline: nested_cargo::Deadline::starting_now(period),
            query_limit: period,
        }
    }
}

/// Describe the family from resolved `inputs`, charging `query` to `budget`.
///
/// # Errors
///
/// Returns the target-directory error from the description, the deadline's
/// refusal when no budget is left for the query, and the query's own error.
pub(super) fn described_from<Q>(inputs: Inputs, budget: Budget, query: Q) -> Result<Described>
where
    Q: FnOnce(Duration) -> Result<Vec<String>>,
{
    // The query's own limit and the budget are both respected: the deadline
    // decides whether there is time to start, and the per-query bound keeps one
    // contended query from consuming the probes' share.
    //
    // Taken first, before the description is built. Building it can touch the
    // filesystem — a path-valued `CARGO_BUILD_TARGET` is canonicalized — and
    // work placed between a caller starting its budget and this check is work
    // that can spend the budget before the query is ever offered it.
    let bound = budget
        .deadline
        .cap(budget.query_limit, "the fixture dependency closure")
        .wrap_err("the timed CLI gate has no budget left for its graph query")?;
    // And the query runs at once, before anything else is done. A bound that
    // waited behind the description would be stale by however long that took,
    // and the query could then start after the deadline it was sized from.
    let owned = owned_units(query(bound)?);
    let (child_env, target_root, hash) = inputs;
    let mut preparation = nested_cargo::Preparation::new(
        nested_cargo::Sources::new("cargo-bdd-cli", env!("CARGO_PKG_VERSION"), hash),
        nested_cargo::Directories::new(
            target_root,
            // The fixture's own directory, because that is where every nested
            // `cargo bdd steps` for this family runs. Cargo resolves a relative
            // `CARGO_HOME` and a relative `RUSTC` against the command's
            // directory, so recording the stamped target root instead would
            // have the identity probe read a different tree than the build.
            fixture_dir(),
        ),
        child_env.clone(),
    )?;
    preparation.owned_packages = owned;
    Ok((preparation, child_env, budget.deadline))
}

/// The timed gate, from one `period` through the query to the probes.
///
/// The budget starts before the inputs are resolved, so the whole gate is
/// inside it. The closure query and both identity probes are charged against
/// it, so no later command starts with an allowance the earlier ones spent.
///
/// # Errors
///
/// As [`cli_preparation`] and [`gate_from`].
pub(super) fn gate_within<Q>(period: Duration, under_nextest: bool, query: Q) -> Result<()>
where
    Q: FnOnce(Duration) -> Result<Vec<String>>,
{
    let budget = Budget::starting_now(period);
    gate_from(cli_preparation()?, budget, under_nextest, query)
}

/// [`gate_within`] from inputs already resolved and a budget already running.
///
/// This is where the deadline the query was charged against is handed to the
/// gate, so it is the function the hand-off regression drives.
///
/// # Errors
///
/// As [`described_from`], and the gate's refusal or preparation error.
pub(super) fn gate_from<Q>(
    inputs: Inputs,
    budget: Budget,
    under_nextest: bool,
    query: Q,
) -> Result<()>
where
    Q: FnOnce(Duration) -> Result<Vec<String>>,
{
    let (preparation, child_env, deadline) = described_from(inputs, budget, query)?;
    ensure_gated_with(&preparation, under_nextest, deadline, || {
        nested_build::capture_preparation(&child_env)
    })
}
