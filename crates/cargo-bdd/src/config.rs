//! CLI configuration types, source merging and validated handler settings.
//!
//! Both loaders carry a `cargo_bdd.configuration` span so a tracing
//! subscriber can time configuration loading per command. The spans record
//! only the command name: configuration values, file paths and environment
//! lookups stay out of the trace, and the binary installs no subscriber, so
//! the default command output is unchanged.

use clap::{ArgMatches, Parser};
use eyre::{Context, Result, bail};
use ortho_config::{OrthoConfig, SubcmdConfigMerge};
use serde::{Deserialize, Serialize};

/// `steps` options that can also be loaded from configuration.
#[derive(Debug, Default, Parser, Deserialize, Serialize, OrthoConfig)]
#[command(name = "steps")]
#[ortho_config(prefix = "CARGO_BDD_")]
pub(crate) struct StepsConfig {
    /// Filter for step definitions bypassed when scenarios were skipped.
    #[arg(long)]
    #[ortho_config(cli_default_as_absent)]
    pub(crate) skipped: bool,
    /// Emit JSON instead of human-readable text.
    #[arg(long)]
    #[ortho_config(cli_default_as_absent)]
    pub(crate) json: bool,
}

/// `skipped` options that can also be loaded from configuration.
#[derive(Debug, Default, Parser, Deserialize, Serialize, OrthoConfig)]
#[command(name = "skipped")]
#[ortho_config(prefix = "CARGO_BDD_")]
pub(crate) struct SkippedConfig {
    /// Include file/line information and skip reasons.
    #[arg(long)]
    #[ortho_config(cli_default_as_absent)]
    pub(crate) reasons: bool,
    /// Emit JSON instead of human-readable text.
    #[arg(long)]
    #[ortho_config(cli_default_as_absent)]
    pub(crate) json: bool,
}

/// Validated settings passed from the CLI adapter to the steps handler.
#[derive(Debug, Default, Eq, PartialEq)]
pub(crate) struct StepsSettings {
    /// Whether to list steps bypassed because scenarios were skipped.
    pub(crate) skipped: bool,
    /// Whether to write the selected step list as JSON.
    pub(crate) json: bool,
}

/// Validated settings passed from the CLI adapter to the skipped handler.
#[derive(Debug, Default, Eq, PartialEq)]
pub(crate) struct SkippedSettings {
    /// Whether to include source locations and skip reasons.
    pub(crate) reasons: bool,
    /// Whether to write skipped scenarios as JSON.
    pub(crate) json: bool,
}

/// Merge `steps` settings from process sources.
#[tracing::instrument(name = "cargo_bdd.configuration", skip_all, fields(command = "steps"))]
pub(crate) fn merge_steps(config: &StepsConfig, matches: &ArgMatches) -> Result<StepsConfig> {
    config
        .load_and_merge_with_matches(matches)
        .wrap_err("failed to load configuration for `steps`")
}

/// Load `skipped` settings from process sources.
#[tracing::instrument(
    name = "cargo_bdd.configuration",
    skip_all,
    fields(command = "skipped")
)]
pub(crate) fn resolve_skipped(
    config: &SkippedConfig,
    matches: &ArgMatches,
) -> Result<SkippedSettings> {
    let merged = config
        .load_and_merge_with_matches(matches)
        .wrap_err("failed to load configuration for `skipped`")?;
    Ok(SkippedSettings {
        reasons: merged.reasons,
        json: merged.json,
    })
}

/// Reject a file or environment combination that the CLI alone cannot express.
pub(crate) fn validate_steps(config: &StepsConfig) -> Result<StepsSettings> {
    if config.json && !config.skipped {
        bail!(concat!(
            "invalid configuration `cmds.steps.json = true`: set ",
            "`cmds.steps.skipped = true` or pass `--skipped`"
        ));
    }
    Ok(StepsSettings {
        skipped: config.skipped,
        json: config.json,
    })
}

#[cfg(test)]
#[path = "config/tests.rs"]
mod tests;
