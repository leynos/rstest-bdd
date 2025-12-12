//! Structured logging with environment variable configuration.
//!
//! This module initialises the logging subsystem for the language server.
//! Logs are written to stderr to avoid interfering with JSON-RPC communication
//! on stdout.

use tracing_subscriber::EnvFilter;
use tracing_subscriber::fmt::format::FmtSpan;

use crate::config::ServerConfig;

fn filter_from_config(config: &ServerConfig) -> EnvFilter {
    EnvFilter::new(config.log_level.as_filter_str())
}

/// Initialise the logging subsystem based on configuration.
///
/// Sets up tracing with the specified log level. Logs are written to stderr
/// to avoid interfering with the JSON-RPC communication on stdout.
///
/// # Environment Variables
///
/// Log level precedence (highest to lowest):
///
/// 1. CLI `--log-level` (parsed into `config.log_level`)
/// 2. `RSTEST_BDD_LSP_LOG_LEVEL` (parsed into `config.log_level`)
/// 3. Default configuration value
///
/// # Note
///
/// If a global subscriber is already set, this function silently ignores
/// the error. This is expected behaviour in tests or when multiple
/// components attempt to initialise logging.
pub fn init_logging(config: &ServerConfig) {
    let filter = filter_from_config(config);

    let subscriber = tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(std::io::stderr)
        .with_span_events(FmtSpan::CLOSE)
        .with_target(true)
        .with_thread_ids(false)
        .with_file(false)
        .with_line_number(false)
        .finish();

    // Ignore error if a subscriber is already set (e.g., in tests).
    // The first subscriber wins, which is the expected behaviour.
    let _ = tracing::subscriber::set_global_default(subscriber);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::LogLevel;

    #[test]
    fn init_logging_does_not_panic() {
        let config = ServerConfig::default();
        init_logging(&config);
    }

    #[test]
    fn init_logging_is_idempotent() {
        let config = ServerConfig::default();
        init_logging(&config);
        init_logging(&config);
    }

    #[test]
    fn filter_uses_config_log_level() {
        let config = ServerConfig::default().with_log_level(LogLevel::Debug);
        let filter = filter_from_config(&config);
        assert_eq!(filter.to_string(), "debug");
    }
}
