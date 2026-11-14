//! Result shapes produced by the shared harness and asserted by the scenario
//! steps.
//!
//! The shapes mirror the two-step split of the experiment: the dep-info
//! slice (a direct filesystem contract, cheap) and the edit-and-rebuild slice
//! (the end-to-end corroboration, expensive). Each carries the captured
//! output so a failing assertion can reproduce the run verbatim.
//!
//! Every shape also has an `abandoned` constructor for the run that never
//! started. A harness step can fail before it produces any observation — the
//! fixture is not prepared, or the baseline could not be launched — and each
//! such path needs the same "nothing was observed, here is why" value. Naming
//! that value once keeps the reason in `baseline_error` and the unobserved
//! flags at their failing default, instead of repeating the whole literal at
//! every early return.

/// Outcome of the baseline-build slice of the experiment: the fixture is
/// compiled with `--no-run`, its dependency-test binary is located through
/// `--message-format=json`, and its rustc dep-info is read.
pub(crate) struct DepInfoOutcome {
    /// Number of times the `#[scenario]`-bound feature file appears in the
    /// test binary's `.d` primary rule.
    pub(crate) dep_info_entry_count: usize,
    /// Whether the `scenarios!` directory's *filtered-out* feature file
    /// (`no_match.feature`, excluded by a `tags =` filter) is listed in the
    /// same primary rule — a parsed-but-unmatched file must still be tracked.
    pub(crate) scenarios_no_match_tracked: bool,
    /// The `.d` content, for failure reporting.
    pub(crate) dep_info_sample: String,
    /// The resolved child environment, for failure reporting.
    pub(crate) child_env_detail: String,
    /// A human-readable reason when the baseline build failed to run.
    pub(crate) baseline_error: Option<String>,
}

impl DepInfoOutcome {
    /// The observation of a run that never reached a build.
    ///
    /// Every count and flag stays at its failing default, so the scenario
    /// steps report a missing observation as the failure it is, and `reason`
    /// travels to the assertion message through `baseline_error`.
    pub(crate) fn abandoned(reason: String, dep_info_sample: String) -> Self {
        Self {
            dep_info_entry_count: 0,
            scenarios_no_match_tracked: false,
            dep_info_sample,
            child_env_detail: String::new(),
            baseline_error: Some(reason),
        }
    }
}

/// Outcome of the edit-and-rebuild slice of the experiment: the baseline full
/// run must pass, then only the captured expectation in the `.feature` file
/// changes and the next run must recompile and fail against the new
/// expectation.
pub(crate) struct RebuildOutcome {
    /// Whether the pre-edit `cargo test` run succeeded.
    pub(crate) baseline_passed: bool,
    /// The pre-edit run's full output when it failed, for failure reporting.
    pub(crate) baseline_output: String,
    /// The post-edit run's status and output.
    pub(crate) second: SecondRun,
}

impl RebuildOutcome {
    /// The observation of a run that never reached the baseline.
    ///
    /// The baseline is reported as failed — because it did not run — and the
    /// second run as an unobserved default, so no rebuild assertion can pass
    /// on a run that never happened.
    pub(crate) fn abandoned(reason: String) -> Self {
        Self {
            baseline_passed: false,
            baseline_output: reason,
            second: SecondRun::unobserved(),
        }
    }
}

/// The post-edit run: its status, whether it recompiled, whether its output
/// names the new expectation, and its full output for failure reporting.
pub(crate) struct SecondRun {
    /// Whether the post-edit run failed.
    pub(crate) failed: bool,
    /// Whether the post-edit stderr contains the `Compiling` line.
    pub(crate) recompiled: bool,
    /// Whether the post-edit output names the new expectation.
    pub(crate) names_new_expectation: bool,
    /// The post-edit run's full output, for failure reporting.
    pub(crate) output: String,
}

impl SecondRun {
    /// The post-edit run that never happened.
    pub(crate) fn unobserved() -> Self {
        Self {
            failed: false,
            recompiled: false,
            names_new_expectation: false,
            output: String::new(),
        }
    }
}
