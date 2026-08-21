//! Result shapes produced by the shared harness and asserted by the scenario
//! steps.
//!
//! The shapes mirror the two-step split of the experiment: the dep-info
//! slice (a direct filesystem contract, cheap) and the edit-and-rebuild slice
//! (the end-to-end corroboration, expensive). Each carries the captured
//! output so a failing assertion can reproduce the run verbatim.

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
