//! CLI configuration types, source merging and validated handler settings.
//!
//! The resolvers take their file-discovery roots and environment from a
//! [`ConfigSources`] value. The application edge supplies process-backed
//! sources; the unit tests supply isolated ones, so both exercise the same
//! merge and error paths instead of a copy of them.
//!
//! Both loaders carry a `cargo_bdd.configuration` span so a tracing
//! subscriber can time configuration loading per command. The spans record
//! only the command name: configuration values, file paths and environment
//! lookups stay out of the trace. The binary installs a stderr subscriber only
//! when `CARGO_BDD_LOG` is set, so the default command output and exit
//! statuses are unchanged.

use std::{path::PathBuf, sync::Arc};

use clap::{ArgMatches, CommandFactory, Parser};
use eyre::{Context, Result, bail};
use ortho_config::{
    CliValueExtractor,
    EnvSource,
    OrthoConfig,
    OrthoResult,
    ProcessEnv,
    SharedScanEnvSource,
    SubcommandCliMatches,
    SubcommandFileContext,
    load_and_merge_subcommand_for_with_matches_with_sources_at,
    process_scan_env_source,
};
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

/// Discovery roots and environment values one configuration load reads.
///
/// The application edge builds these from the live process with
/// [`ConfigSources::process`]. Tests build them from isolated directories and a
/// fixed environment, so both drive the same resolvers rather than a
/// re-implementation of the merge call.
pub(crate) struct ConfigSources {
    /// Directory the loader treats as the local configuration base.
    base: PathBuf,
    /// Named lookups used to locate the discovery directories.
    discovery: Box<dyn EnvSource>,
    /// Environment values merged over the file layers.
    environment: SharedScanEnvSource,
}

impl ConfigSources {
    /// Read the live process environment, rooted at the working directory.
    pub(crate) fn process() -> Self {
        Self {
            base: PathBuf::from("."),
            discovery: Box::new(ProcessEnv),
            environment: process_scan_env_source(),
        }
    }

    /// Read `base` and `discovery` instead of the process environment.
    ///
    /// Used by the unit tests, which must not depend on a developer's real
    /// configuration files or environment.
    #[cfg(test)]
    pub(crate) fn isolated(
        base: PathBuf,
        discovery: impl EnvSource + 'static,
        environment: SharedScanEnvSource,
    ) -> Self {
        Self {
            base,
            discovery: Box::new(discovery),
            environment,
        }
    }

    /// Borrow the file-search context for one merge.
    fn files(&self) -> SubcommandFileContext<'_> {
        SubcommandFileContext::new(&self.base, self.discovery.as_ref())
    }
}

/// Merge `steps` settings from the supplied sources.
#[tracing::instrument(name = "cargo_bdd.configuration", skip_all, fields(command = "steps"))]
pub(crate) fn merge_steps(
    config: &StepsConfig,
    matches: &ArgMatches,
    sources: &ConfigSources,
) -> Result<StepsConfig> {
    merge_from_sources(config, matches, sources)
        .wrap_err("failed to load configuration for `steps`")
}

/// Load `skipped` settings from the supplied sources.
#[tracing::instrument(
    name = "cargo_bdd.configuration",
    skip_all,
    fields(command = "skipped")
)]
pub(crate) fn resolve_skipped(
    config: &SkippedConfig,
    matches: &ArgMatches,
    sources: &ConfigSources,
) -> Result<SkippedSettings> {
    let merged = merge_from_sources(config, matches, sources)
        .wrap_err("failed to load configuration for `skipped`")?;
    Ok(SkippedSettings {
        reasons: merged.reasons,
        json: merged.json,
    })
}

/// Merge one subcommand's configuration through `ortho_config`.
fn merge_from_sources<T>(
    config: &T,
    matches: &ArgMatches,
    sources: &ConfigSources,
) -> OrthoResult<T>
where
    T: OrthoConfig
        + Serialize
        + serde::de::DeserializeOwned
        + Default
        + CommandFactory
        + CliValueExtractor,
{
    load_and_merge_subcommand_for_with_matches_with_sources_at(
        &SubcommandCliMatches::new(config, matches),
        sources.files(),
        Arc::clone(&sources.environment),
    )
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
